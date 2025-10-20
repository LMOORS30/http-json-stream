use serde::de::DeserializeOwned;
use std::collections::VecDeque;
use std::fmt;
use std::io::{Cursor, Read, Write};
use std::marker::{PhantomData, Unpin};

/// A filter for [`PartialJson`] to determine which items to deserialize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonPart {
    pub level: u32,
    pub group: Option<u32>,
    pub parse_object_values: bool,
    pub ignore_list_entries: bool,
}

impl JsonPart {
    /// The level determines at which depth to deserialize data:
    /// - `0` - the full JSON is parsed as a single value once [`done`](PartialJson::done) is called.
    /// - `1` - all entries inside of the root group are deserialized individually.
    /// - `2` - only entries inside of a group found directly within the root.
    /// - `3` - ...
    ///
    /// A group is an object or a list, both will increase the depth, but entries may be parsed from either or both.
    pub fn level(level: u32) -> Self {
        JsonPart {
            level,
            group: None,
            parse_object_values: false,
            ignore_list_entries: false,
        }
    }
    /// Deserialize only entries found within the group with the given index.
    ///
    /// To deserialize only entries inside of the first list found within the root object:
    /// ```
    /// # use http_json_stream::JsonPart;
    /// JsonPart::level(2).group(0);
    /// ```
    pub fn group(mut self, index: u32) -> Self {
        self.group = Some(index);
        self
    }
    /// Parse object values as if they were list entries, ignoring the keys.
    ///
    /// This influences the index counted by [`group`](JsonPart::group), now also counting objects.
    /// ```
    /// # use http_json_stream::JsonPart;
    /// JsonPart::level(2).parse_object_values();
    /// ```
    pub fn parse_object_values(mut self) -> Self {
        self.parse_object_values = true;
        self
    }
    /// Do not parse list entries and use [`parse_object_values`](JsonPart::parse_object_values) instead.
    ///
    /// This influences the index counted by [`group`](JsonPart::group), now only counting objects.
    /// ```
    /// # use http_json_stream::JsonPart;
    /// JsonPart::level(2).ignore_list_entries();
    /// ```
    pub fn ignore_list_entries(mut self) -> Self {
        self.ignore_list_entries = true;
        self.parse_object_values = true;
        self
    }
}

/// A [`Write`] or [`Extend`] interface to push [`JSON`](serde_json) data.
///
/// An [`Iterator`] interface to pull [`Deserialized`](serde) data.
///
/// ```
/// # use std::io::Write;
/// # use std::marker::Unpin;
/// # use futures_core::Stream;
/// # use futures_util::StreamExt;
/// # use serde::de::DeserializeOwned;
/// # use http_json_stream::{JsonPart, PartialJson};
/// # fn process_data<T>(_: T) {}
/// async fn parse_json_stream<S, T>(mut stream: S)
/// where
///     S: Stream<Item = Vec<u8>> + Unpin,
///     T: DeserializeOwned,
/// {
///     let mut json = PartialJson::<T>::new(JsonPart::level(1).group(0));
///     while let Some(chunk) = stream.next().await {
///         json.extend(&chunk);
///         while let Some(item) = json.next() {
///             process_data(item);
///         }
///     }
///     if let Some(item) = json.done() {
///         process_data(item);
///     }
/// }
/// ```
pub struct PartialJson<T> {
    buffer: VecDeque<u8>,
    part: JsonPart,
    level: u32,
    group: u32,
    item_char: char,
    last_char: char,
    in_string: bool,
    i: usize,
    phantom: PhantomData<T>,
}

impl<T> PartialJson<T> {
    /// Creates a new JSON parser which can deserialize data as it is being written.
    ///
    /// The data to be deserialized is determined by the [`JsonPart`] filter.
    pub fn new(part: JsonPart) -> Self {
        PartialJson {
            buffer: VecDeque::new(),
            part,
            level: 0,
            group: 0,
            item_char: '\0',
            last_char: '\0',
            in_string: false,
            i: 0,
            phantom: PhantomData,
        }
    }
}

impl<T> Clone for PartialJson<T> {
    fn clone(&self) -> Self {
        PartialJson {
            buffer: self.buffer.clone(),
            part: self.part,
            level: self.level,
            group: self.group,
            item_char: self.item_char,
            last_char: self.last_char,
            in_string: self.in_string,
            i: self.i,
            phantom: self.phantom,
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.buffer.clone_from(&source.buffer);
        self.part = source.part;
        self.level = source.level;
        self.group = source.group;
        self.item_char = source.item_char;
        self.last_char = source.last_char;
        self.in_string = source.in_string;
        self.i = source.i;
        self.phantom = source.phantom;
    }
}

unsafe impl<T> Send for PartialJson<T> {}
unsafe impl<T> Sync for PartialJson<T> {}
impl<T> Unpin for PartialJson<T> {}

impl<T> fmt::Debug for PartialJson<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PartialJson").field(&self.buffer).finish()
    }
}

impl<T> Write for PartialJson<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<T> Extend<u8> for PartialJson<T> {
    fn extend<I: IntoIterator<Item = u8>>(&mut self, iter: I) {
        self.buffer.extend(iter);
    }
}

impl<'a, T> Extend<&'a u8> for PartialJson<T> {
    fn extend<I: IntoIterator<Item = &'a u8>>(&mut self, iter: I) {
        self.buffer.extend(iter);
    }
}

impl<T: DeserializeOwned> Iterator for PartialJson<T> {
    type Item = crate::Result<T>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.i == self.buffer.len() {
                if !self.buffer.is_empty() {
                    let skipping = self.level < self.part.level;
                    if skipping || self.ignored() {
                        self.drain();
                    }
                }
                return None;
            }
            let char = self.buffer[self.i] as char;
            self.i += 1;
            if self.in_string {
                if self.last_char == '\\' {
                    self.last_char = '\0';
                } else {
                    if char == '"' {
                        self.in_string = false;
                    }
                    self.last_char = char;
                }
            } else if !char.is_ascii_whitespace() {
                if let '"' = char {
                    self.in_string = true;
                } else if let '[' | '{' = char {
                    self.level += 1;
                    if self.grounded() {
                        self.item_char = char;
                    }
                } else if let ',' = char {
                    if self.grounded() && !self.ignored() {
                        return Some(self.parse(self.i - 1));
                    }
                } else if let ']' | '}' = char {
                    let grounded = self.grounded();
                    let ignored_index = self.ignored_index();
                    let ignored_group = self.ignored_group();
                    self.level = self.level.saturating_sub(1);
                    if grounded {
                        if !ignored_group {
                            self.group += 1;
                        }
                        let empty = matches!(self.last_char, '[' | '{');
                        if !ignored_index && !ignored_group && !empty {
                            return Some(self.parse(self.i - 1));
                        }
                    }
                }
                if let '[' | ':' = char {
                    if self.grounded() {
                        self.drain();
                    }
                }
                self.last_char = char;
            }
        }
    }
}

impl<T: DeserializeOwned> PartialJson<T> {
    /// Does nothing and returns `None` if not currently on the correct [`level`](JsonPart::level).
    ///
    /// Otherwise, clears the full buffer and attempts to deserialize an item.
    ///
    /// Intended for use once writing is finished and the level might be zero.
    pub fn done(&mut self) -> Option<crate::Result<T>> {
        if self.grounded() && !self.ignored() {
            Some(self.parse(self.buffer.len()))
        } else {
            None
        }
    }
}

impl<T: DeserializeOwned> PartialJson<T> {
    fn parse(&mut self, j: usize) -> crate::Result<T> {
        let (one, two) = self.buffer.as_slices();
        let res = {
            if one.len() < j {
                let j = j - one.len();
                serde_json::from_reader(Cursor::new(one).chain(Cursor::new(&two[..j])))
            } else {
                serde_json::from_slice(&one[..j])
            }
        };
        let result = res.map_err(|err| {
            let buf = self.buffer.iter().take(j).copied().collect::<Vec<_>>();
            crate::Error::Json(err, String::from_utf8(buf))
        });
        self.drain();
        result
    }
    #[inline]
    fn grounded(&self) -> bool {
        self.level == self.part.level
    }
    #[inline]
    fn ignored(&self) -> bool {
        self.ignored_index() || self.ignored_group()
    }
    #[inline]
    fn ignored_index(&self) -> bool {
        self.part.group.is_some_and(|group| group != self.group)
    }
    #[inline]
    fn ignored_group(&self) -> bool {
        let ignored_object = !self.part.parse_object_values && self.item_char == '{';
        let ignored_list = self.part.ignore_list_entries && self.item_char == '[';
        ignored_object || ignored_list
    }
    #[inline]
    fn drain(&mut self) {
        self.buffer.drain(..self.i);
        self.i = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde::Deserialize;
    use std::fmt::Debug;

    fn lvl(lvl: u32) -> JsonPart {
        JsonPart::level(lvl).parse_object_values()
    }

    #[track_caller]
    fn run<T>(part: JsonPart, str: &str, obj: &[T])
    where
        T: Debug + PartialEq + Eq + DeserializeOwned,
    {
        for len in 1..=str.len() {
            let mut res = Vec::new();
            let mut json = PartialJson::<T>::new(part);
            for chunk in str.as_bytes().chunks(len) {
                json.extend(chunk);
                while let Some(item) = json.next() {
                    res.push(item.unwrap());
                }
            }
            if let Some(item) = json.done() {
                res.push(item.unwrap());
            }
            assert_eq!(res, obj);
        }
    }

    #[test]
    fn empty() {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct Empty {}
        run::<()>(lvl(1), "{\n }", &[]);
        run::<()>(lvl(1), "[ \n]", &[]);
        run::<Empty>(lvl(0), "{ \n}", &[Empty {}]);
        run::<Vec<()>>(lvl(0), "[\n ]", &[vec![]]);
        run::<Vec<()>>(lvl(1).group(0), "{ \"\": [ \n] }", &[vec![]]);
        run::<Vec<()>>(JsonPart::level(1), "{ \"\": [ \n] }", &[]);
        run::<Vec<()>>(lvl(1).group(1), "{ \"\": [ \n] }", &[]);
        run::<()>(lvl(2), "{ \"\": [ \n] }", &[]);
    }

    #[test]
    fn simple() {
        let list = "[[ ],[1,2, 3], [], [4,5], []]";
        let map = r#"{ "\"":[1,2],"[":[3, 4,5]}"#;
        run(lvl(2), list, &[1, 2, 3, 4, 5]);
        run(lvl(2), map, &[1, 2, 3, 4, 5]);
        run(
            JsonPart::level(1),
            list,
            &[vec![], vec![1, 2, 3], vec![], vec![4, 5], vec![]],
        );
        run(
            JsonPart::level(1).ignore_list_entries(),
            map,
            &[vec![1, 2], vec![3, 4, 5]],
        );
        run(lvl(2).group(3), list, &[4, 5]);
        run(lvl(2).group(1), map, &[3, 4, 5]);
        run::<()>(JsonPart::level(2).ignore_list_entries(), list, &[]);
    }

    #[test]
    fn objects() {
        #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
        struct Item {
            a: String,
            b: Vec<u32>,
        }
        let json = r#"[
            {"list": [
                { "b": [3, 4], "a": "test2"},
                { "a": "test", "b": [1, 2]}
            ]},
            {"one": {
                "a": { "b": [1, 2], "a": "test"}
            ],
            "two": [
                { "a": "test2", "b": [3, 4]}
            ]},
            {"more": {
                "b": { "b": [3, 4], "a": "test2"},
                "c": { "a": "test", "b": [1, 2]}
            }}
        ]"#;
        let one = Item {
            a: "test".into(),
            b: vec![1, 2],
        };
        let two = Item {
            a: "test2".into(),
            b: vec![3, 4],
        };
        run(
            lvl(3),
            json,
            &[
                two.clone(),
                one.clone(),
                one.clone(),
                two.clone(),
                two.clone(),
                one.clone(),
            ],
        );
        run(
            JsonPart::level(3),
            json,
            &[two.clone(), one.clone(), two.clone()],
        );
        run(
            JsonPart::level(3).ignore_list_entries(),
            json,
            &[one.clone(), two.clone(), one.clone()],
        );
        run(
            JsonPart::level(3).ignore_list_entries().group(1),
            json,
            &[two.clone(), one.clone()],
        );
        run(JsonPart::level(3).group(1), json, &[two.clone()]);
        run(lvl(3).group(0), json, &[two.clone(), one.clone()]);
        run(lvl(3).group(1), json, &[one.clone()]);
        run(lvl(3).group(2), json, &[two.clone()]);
        run(lvl(3).group(3), json, &[two.clone(), one.clone()]);
        run::<Item>(lvl(3).group(4), json, &[]);
        let none = JsonPart {
            level: 3,
            group: None,
            ignore_list_entries: true,
            parse_object_values: false,
        };
        run::<Item>(none, json, &[]);
    }

    #[test]
    fn escape() {
        run(lvl(1), r#"["\\"]"#, &["\\".to_string()]);
        run(lvl(1), r#"["\\\", ", "\\"]"#, &["\\\", ".to_string(), "\\".to_string()]);
        run(lvl(1), r#"["\\\n", ", \\", "\\n"]"#, &["\\\n".to_string(), ", \\".to_string(), "\\n".to_string()]);
    }

    #[test]
    fn utf8() {
        run(
            lvl(2),
            r#"{
                "ⓟH񠭇򓊦򤄛⅃ތfǋͨ񣧺r抄ݼ춓\"ĠQ𭌔\󛞼ܫ􂷳쯪􁺬ڹދ􅽘͹៦9ު": [
                    "󸈣ظȗ򷇪\"𐏴駭󳷈맪ى퇄<}i񩇦򭍁ި𥏡p;0+􆨧ͅд򔦜j솞U_.ߤb",
                    ":˚�梔񂌄֪Ͻv񴷤Ẑ򲬧D񔰾衸򊻤LY�\"䋪ω`񱰲󞰅怟񤹝zw󐭄#ͦ"
                ],
                "𬻈;:䓄$ۺ䰁%୯NYꊠ|㦴⢗$X㴩߉񫤵؊񇢏\"9׳󯠀ߊvƉA񾩲ҁ֋": "릪𖲨ѯ⦤͛𞊎͐4륛𪮫Θ񈐦𐃇\"򕘾򚂼澀񿡬ꖆߐͤwߥ󉙥뉧ƕ󈀁[ɟϏ叓D񧿾",
                "瑔샂,󂦆򤍟򕤕͘꯯ϒނ\"𖄪𽞲𼤊ب𔽢ʌ鋂BϑÂ򗴎󹜦瑜񅃼󁲫봗Lk󊣧ԣ旧": [
                    "Z̩ťMɲ髲򒝪񠛱ቱ̗桒\"𾤄W𴊖􀕡򰅼돭_񒐽񻔪䍙Ғѿ򛁬B6ݍ𤦐鄎^Eˍ",
                    "E넰׻𨆣{2ܗ鞬識򋫄󶁇촗'㟳󴧁)#Ş\"ߟ൴򘽍󾥘ڋ⸏ŗ𣻡𝝙ݡi斏ᾝ՞"
                ]
            }"#,
            &[
                "󸈣ظȗ򷇪\"𐏴駭󳷈맪ى퇄<}i񩇦򭍁ި𥏡p;0+􆨧ͅд򔦜j솞U_.ߤb".to_string(),
                ":˚�梔񂌄֪Ͻv񴷤Ẑ򲬧D񔰾衸򊻤LY�\"䋪ω`񱰲󞰅怟񤹝zw󐭄#ͦ".to_string(),
                "Z̩ťMɲ髲򒝪񠛱ቱ̗桒\"𾤄W𴊖􀕡򰅼돭_񒐽񻔪䍙Ғѿ򛁬B6ݍ𤦐鄎^Eˍ".to_string(),
                "E넰׻𨆣{2ܗ鞬識򋫄󶁇촗'㟳󴧁)#Ş\"ߟ൴򘽍󾥘ڋ⸏ŗ𣻡𝝙ݡi斏ᾝ՞".to_string(),
            ],
        )
    }
}
