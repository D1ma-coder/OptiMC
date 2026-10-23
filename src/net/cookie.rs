//! Server-owned opaque cookies, retained across configuration/play transitions.
use super::codec::{read_string, read_varint, string, varint};
use anyhow::{ensure, Result};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Store {
    values: BTreeMap<String, Vec<u8>>,
}
impl Store {
    pub fn store(&mut self, mut data: &[u8]) -> Result<()> {
        let key = read_string(&mut data, 32767)?;
        let size = read_varint(&mut data)?;
        ensure!(
            (0..=5120).contains(&size) && data.len() == size as usize,
            "Invalid server cookie payload"
        );
        ensure!(
            self.values.contains_key(&key) || self.values.len() < 256,
            "Too many server cookies"
        );
        let total: usize = self.values.values().map(Vec::len).sum();
        let old = self.values.get(&key).map_or(0, Vec::len);
        ensure!(
            total - old + data.len() <= 1024 * 1024,
            "Server cookie storage limit exceeded"
        );
        self.values.insert(key, data.to_vec());
        Ok(())
    }
    pub fn response(&self, mut request: &[u8]) -> Result<Vec<u8>> {
        let key = read_string(&mut request, 32767)?;
        ensure!(request.is_empty(), "Trailing cookie request bytes");
        let mut out = Vec::new();
        string(&key, &mut out);
        if let Some(value) = self.values.get(&key) {
            out.push(1);
            varint(value.len() as i32, &mut out);
            out.extend(value);
        } else {
            out.push(0);
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_cookie_round_trip_and_invalid_update_preserves_previous() {
        let mut store = Store::default();
        let mut key = Vec::new();
        string("optimc:opaque", &mut key);
        assert_eq!(
            store.response(&key).unwrap(),
            [key.clone(), vec![0]].concat()
        );
        let bytes = [0, 255, 1, 128];
        let mut packet = key.clone();
        varint(bytes.len() as i32, &mut packet);
        packet.extend(bytes);
        store.store(&packet).unwrap();
        let response = [key.clone(), vec![1, 4], bytes.to_vec()].concat();
        assert_eq!(store.response(&key).unwrap(), response);
        packet.push(0);
        assert!(store.store(&packet).is_err());
        assert_eq!(store.response(&key).unwrap(), response);
        let mut big = key.clone();
        varint(5121, &mut big);
        big.resize(big.len() + 5121, 0);
        assert!(store.store(&big).is_err());
    }
}
