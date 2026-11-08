//! Fuzz `Table` with an arbitrary sequence of operations, checked against a plain model.
//!
//! Each input byte pair is one operation: insert, get, get_mut, remove, or iter_from, aimed at a
//! name the table minted earlier (live or removed) or at an arbitrary word from the input. After
//! every operation the target holds the table to what the proofs promise: a removed name never
//! resolves again, the live names are distinct, an entry holds the name it was minted with, and
//! `len` equals the number of entries a walk finds and the number the model holds.

#![no_main]

use generational_table::{EntryName, Table};
use libfuzzer_sys::fuzz_target;

/// Few slots, so the table fills, empties and reuses slots within a short input.
const SLOTS: usize = 8;

fuzz_target!(|data: &[u8]| {
    let mut table: Table<(u64, u32), SLOTS> = Table::new();
    // Every name ever minted, with the value last stored under it, or `None` once removed.
    let mut minted: Vec<(u64, Option<u32>)> = Vec::new();
    let mut counter = 0_u32;

    let mut bytes = data.chunks_exact(2);
    for pair in &mut bytes {
        let (op, arg) = (pair[0] % 5, pair[1]);
        // The target of a lookup: a name minted before, or (high bit) a word taken from the input.
        let target = if arg & 0x80 == 0 && !minted.is_empty() {
            minted[usize::from(arg) % minted.len()].0
        } else {
            u64::from(arg) | (u64::from(pair[0]) << 32)
        };
        let name = EntryName::from_bits(target);
        let modeled = minted.iter().position(|(bits, _)| *bits == target);
        let live = modeled.and_then(|i| minted[i].1);

        match op {
            0 => {
                counter = counter.wrapping_add(1);
                let value = counter;
                let before = table.len();
                match table.insert_with(|n| (n.to_bits(), value)) {
                    Some(new) => {
                        assert_eq!(table.len(), before + 1);
                        assert!(
                            !minted.iter().any(|(bits, _)| *bits == new.to_bits()),
                            "a name was minted twice"
                        );
                        minted.push((new.to_bits(), Some(value)));
                    }
                    None => assert_eq!(before, SLOTS, "refused with a free slot"),
                }
            }
            1 => {
                let got = table.get(name).map(|e| e.1);
                assert_eq!(got, live);
            }
            2 => {
                if let Some(entry) = table.get_mut(name) {
                    assert_eq!(Some(entry.1), live);
                    counter = counter.wrapping_add(1);
                    entry.1 = counter;
                    minted[modeled.unwrap()].1 = Some(counter);
                } else {
                    assert_eq!(live, None);
                }
            }
            3 => {
                let removed = table.remove(name).map(|e| e.1);
                assert_eq!(removed, live);
                if let Some(i) = modeled {
                    minted[i].1 = None;
                }
                // Gone for good: no operation resolves it again.
                assert!(table.get(name).is_none());
                assert!(table.get_mut(name).is_none());
                assert!(table.remove(name).is_none());
            }
            _ => {
                let from = usize::from(arg) % (SLOTS + 2);
                let mut last = None;
                for (slot, n, entry) in table.iter_from(from) {
                    assert!(slot >= from && slot < SLOTS);
                    assert!(last.is_none_or(|l| l < slot), "slots not ascending");
                    last = Some(slot);
                    assert_eq!(n.to_bits(), entry.0, "entry holds a different name");
                    assert_eq!(table.slot_of(n), Some(slot));
                }
            }
        }

        // Invariants after every operation.
        let walked: Vec<(u64, u32)> = table.iter().map(|(n, e)| (n.to_bits(), e.1)).collect();
        assert_eq!(table.len(), walked.len(), "len differs from the walk");
        assert_eq!(table.is_empty(), walked.is_empty());
        let modeled_live: Vec<(u64, u32)> = minted
            .iter()
            .filter_map(|(bits, v)| v.map(|v| (*bits, v)))
            .collect();
        assert_eq!(
            table.len(),
            modeled_live.len(),
            "len differs from the model"
        );
        let mut names: Vec<u64> = walked.iter().map(|w| w.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), walked.len(), "two live entries share a name");
        for (bits, value) in &modeled_live {
            let entry = table
                .get(EntryName::from_bits(*bits))
                .expect("live name lost");
            assert_eq!((entry.0, entry.1), (*bits, *value));
        }
        for (bits, v) in &minted {
            if v.is_none() {
                assert!(
                    table.get(EntryName::from_bits(*bits)).is_none(),
                    "removed name resolves"
                );
            }
        }
    }
});
