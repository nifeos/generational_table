//! Fuzz `Table` lookups with arbitrary 64-bit words as names.
//!
//! The first byte sets how many entries to insert and the next how many to remove again, so the
//! table holds live, removed and never-minted slots. Every following eight bytes are a word turned
//! into a name with `EntryName::from_bits`. A word the table never minted must resolve to nothing
//! or to its own entry, never to another one, and a word that resolved must remove to exactly the
//! entry it showed and never resolve again.

#![no_main]

use generational_table::{EntryName, Table};
use libfuzzer_sys::fuzz_target;

const SLOTS: usize = 16;

fuzz_target!(|data: &[u8]| {
    let Some((&inserts, rest)) = data.split_first() else {
        return;
    };
    let Some((&removals, rest)) = rest.split_first() else {
        return;
    };
    let mut table: Table<u64, SLOTS> = Table::new();
    let mut minted = Vec::new();
    for _ in 0..inserts % 20 {
        if let Some(name) = table.insert_with(|n| n.to_bits()) {
            minted.push(name);
        }
    }
    for i in 0..usize::from(removals % 20) {
        if let Some(&name) = minted.get(i * 3 % minted.len().max(1)) {
            table.remove(name);
        }
    }

    for word in rest.chunks_exact(8) {
        let bits = u64::from_le_bytes(word.try_into().unwrap());
        let name = EntryName::from_bits(bits);
        let len = table.len();
        match table.get(name).copied() {
            None => {
                assert!(table.get_mut(name).is_none());
                assert!(table.remove(name).is_none());
                assert_eq!(table.len(), len);
            }
            Some(own) => {
                assert_eq!(own, bits, "a word resolved to another entry");
                assert_eq!(table.remove(name), Some(bits));
                assert_eq!(table.len(), len - 1);
                assert!(table.get(name).is_none(), "a removed name resolved again");
            }
        }
        assert_eq!(table.len(), table.iter().count());
        assert_eq!(table.len(), table.iter_from(0).count());
    }
});
