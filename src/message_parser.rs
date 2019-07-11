pub struct MessageParser<'a> {
    original: &'a str,
    offset: usize,
}

impl<'a> MessageParser<'a> {
    pub fn new(s: &'a str) -> MessageParser<'a> {
        MessageParser {
            original: s,
            offset: 0,
        }
    }
}

impl MessageParser<'_> {
    pub fn str_until_any(&self, chars: &[u8]) -> Option<(usize, u8)> {
        for (index, b) in self.original[self.offset..].bytes().enumerate() {
            if chars.contains(&b) {
                return Some((index, b));
            }
        }
        None
    }

    pub fn str_until(&self, c: u8) -> Option<usize> {
        self.str_until_any(&[c]).map(|(i, _)| i)
    }

    pub fn consume_word_until(&mut self, c: u8) -> Option<&str> {
        let index = self.str_until(c)?;
        let start = self.offset;
        let end = start + index;
        let result = &self.original[start..end];
        self.offset = end + 1;
        Some(result)
    }

    pub fn remaining(&self) -> &str {
        &self.original[self.offset..].trim()
    }
}
