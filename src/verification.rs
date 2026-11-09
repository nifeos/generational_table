//! Machine-checked proofs, with Kani. These are the properties a caller that stores names in
//! long-lived records leans on. A two-slot table exhibits every case that matters: the slot in
//! question, a different slot, and reuse.

use super::*;

/// Falsification: replayable `falsifications/verification.a_removed_name_never_resolves_again.patch`
/// A removed name never resolves again, even after its slot is reused. This is the
/// safety a never-reused counter would give, kept under reuse: the
/// old name and the new occupant's name differ (the generation moved), so `get`, `get_mut`,
/// and `remove` all refuse the old one. The table starts at any generations at all, the last
/// one before retirement and a retired slot included, so this holds for the 2^32nd reuse as
/// for the first; [`a_removed_name_stays_dead_whatever_comes_after`] carries it forward.
#[kani::proof]
fn a_removed_name_never_resolves_again() {
    let mut t: Table<u8, 2> = Table::new();
    t.generations = [kani::any(), kani::any()];
    kani::cover!(
        t.generations[0] == Table::<u8, 2>::RETIRED - 1,
        "the last occupant a slot takes"
    );
    let Some(name) = t.insert_with(|_| kani::any()) else {
        return;
    };
    t.remove(name);

    // Reuse the slot: the new occupant must not be reachable through the dead name.
    let reused = t.insert_with(|_| kani::any());
    if let Some(new_name) = reused {
        assert_ne!(new_name, name);
    }
    assert!(t.get(name).is_none());
    assert!(t.get_mut(name).is_none());
    assert!(t.remove(name).is_none());
}

/// A table in any state the operations can reach, for N = 2: each slot free or live, with
/// any generation, except that a live slot is never at `RETIRED`. `live` and `top` follow
/// from the slots, as insert and remove keep them.
fn any_reachable_table() -> Table<u8, 2> {
    let mut t: Table<u8, 2> = Table::new();
    for slot in 0..2 {
        t.generations[slot] = kani::any();
        if kani::any() {
            kani::assume(t.generations[slot] != Table::<u8, 2>::RETIRED);
            t.slots[slot] = Some(kani::any());
            t.live += 1;
            t.top = slot + 1;
        }
    }
    t
}

/// One insert, one remove of any name, or one clear: every operation that changes a slot or a
/// generation.
fn any_operation(t: &mut Table<u8, 2>) {
    match kani::any::<u8>() {
        0 => {
            let _ = t.insert_with(|_| kani::any());
        }
        1 => t.clear(),
        _ => {
            let _ = t.remove(EntryName::from_bits(kani::any()));
        }
    }
}

/// Falsification: replayable `falsifications/verification.a_removed_name_stays_dead_whatever_comes_after.patch`
/// A dead name stays dead whatever operations follow, with no bound on how many. The
/// induction step: from any reachable table and any name whose generation is below its slot's,
/// which is what a removal leaves behind, two arbitrary operations keep the slot's generation
/// above the name's and every live slot below `RETIRED`, and the name resolves nowhere. Both
/// facts are what the step assumes, so they hold after any number of operations, and a slot
/// reused 2^32 times cannot bring an old name back. Two operations, not one, because a
/// wrapping table needs an insert and a remove to come round.
#[kani::proof]
fn a_removed_name_stays_dead_whatever_comes_after() {
    let mut t = any_reachable_table();
    let slot: usize = kani::any();
    kani::assume(slot < 2);
    let generation: u32 = kani::any();
    kani::assume(generation < t.generations[slot]);
    let dead = Table::<u8, 2>::name(slot, generation);
    kani::cover!(
        t.generations[slot] == Table::<u8, 2>::RETIRED && t.slots[slot].is_none(),
        "a retired slot"
    );

    for _ in 0..2 {
        any_operation(&mut t);
        assert!(
            generation < t.generations[slot],
            "a slot's generation came back down"
        );
        for s in 0..2 {
            assert!(t.slots[s].is_none() || t.generations[s] != Table::<u8, 2>::RETIRED);
        }
        assert!(t.get(dead).is_none());
    }
}

/// Falsification: replayable `falsifications/verification.the_walk_bound_and_the_live_count_survive_any_operation.patch`
/// The bookkeeping every walk and `len` lean on survives any operation, with no bound on how
/// many. The induction step: from any reachable table, one arbitrary operation leaves `top` exactly
/// one past the highest live slot (so every slot at or above it is empty), and `live` equal to the
/// number of occupied slots. Those are the facts [`any_reachable_table`] assumes, so they hold after
/// any number of operations, and every walk bounded by `top` reports exactly `len` entries, not
/// only after the fixed sequence of [`a_bounded_walk_yields_every_live_entry`].
#[kani::proof]
#[kani::unwind(3)]
fn the_walk_bound_and_the_live_count_survive_any_operation() {
    let mut t = any_reachable_table();
    any_operation(&mut t);

    assert!(t.top <= 2, "the walk bound passed the capacity");
    assert!(
        t.slots[t.top..].iter().all(Option::is_none),
        "a live entry sits above the walk bound"
    );
    assert!(
        t.top == 0 || t.slots[t.top - 1].is_some(),
        "the walk bound sits above an empty slot"
    );
    assert_eq!(
        t.live,
        t.slots.iter().filter(|s| s.is_some()).count(),
        "the live count drifted from the occupied slots"
    );
    assert_eq!(t.values().count(), t.len());
    assert_eq!(t.values_mut().count(), t.len());
    assert_eq!(t.live_slots().count(), t.len());
    assert_eq!(t.iter_from(0).count(), t.len());

    // `iter_mut` walks the same entries under the same names as `iter`.
    let mut names = [None; 2];
    for (i, (name, _)) in t.iter_mut().enumerate() {
        names[i] = Some(name);
    }
    let mut shared = t.iter().map(|(name, _)| name);
    assert_eq!(names, [shared.next(), shared.next()]);
}

/// Falsification: replayable `falsifications/verification.a_cleared_table_resolves_no_name_it_held.patch`
/// Clear kills every name the table held, for good. From any reachable table and any name, after
/// `clear` the table is empty and walks nothing, the name resolves nowhere, and the entry the next
/// insert takes is never named like an entry the table held before the clear: the cleared slot's
/// generation moved, as a removal moves it.
#[kani::proof]
fn a_cleared_table_resolves_no_name_it_held() {
    let mut t = any_reachable_table();
    let probe = EntryName::from_bits(kani::any());
    let was_live = t.contains_key(probe);
    kani::cover!(was_live, "a live name before the clear");

    t.clear();
    assert!(t.is_empty());
    assert_eq!(t.top, 0, "a cleared table still walks slots");
    assert!(t.slots.iter().all(Option::is_none));
    assert!(t.get(probe).is_none());

    let next = t.insert_with(|_| kani::any());
    if was_live {
        assert_ne!(next, Some(probe), "a cleared name came back");
    }
    assert!(!t.contains_key(probe) || !was_live);
}

/// Falsification: replayable `falsifications/verification.contains_key_and_get_agree_on_any_name.patch`
/// `contains_key` is `get` without the reference: from any reachable table, for any `u64` at all,
/// the two agree, so membership inherits every refusal `get` is proved to make (a removed name, a
/// previous occupant's name, a forged word).
#[kani::proof]
fn contains_key_and_get_agree_on_any_name() {
    let t = any_reachable_table();
    let probe = EntryName::from_bits(kani::any());
    assert_eq!(t.contains_key(probe), t.get(probe).is_some());
}

/// Falsification: replayable `falsifications/verification.live_names_are_distinct_and_resolve_to_their_own_entry.patch`
/// Two live entries never share a name, and each name resolves to its own entry: the
/// packing cannot alias two occupants.
#[kani::proof]
fn live_names_are_distinct_and_resolve_to_their_own_entry() {
    let mut t: Table<u8, 2> = Table::new();
    let (a_value, b_value): (u8, u8) = (kani::any(), kani::any());
    let a = t.insert_with(|_| a_value).unwrap();
    let b = t.insert_with(|_| b_value).unwrap();

    assert_ne!(a, b);
    assert_eq!(t.get(a).copied(), Some(a_value));
    assert_eq!(t.get(b).copied(), Some(b_value));
}

/// Falsification: replayable `falsifications/verification.a_name_the_table_never_minted_resolves_to_nothing.patch`
/// A garbage name resolves to nothing. For any u64 whatsoever, the name `from_bits` makes of it
/// either fails to resolve or is exactly the name the table minted: there is no input that
/// reaches an entry through a name the table never issued. (A name may arrive from an untrusted
/// caller; this is the "cannot forge" claim for names.)
#[kani::proof]
fn a_name_the_table_never_minted_resolves_to_nothing() {
    let mut t: Table<u8, 2> = Table::new();
    let minted = t.insert_with(|_| kani::any());
    let probe = EntryName::from_bits(kani::any());
    if t.get(probe).is_some() {
        assert_eq!(Some(probe), minted);
    }
}

/// Falsification: replayable `falsifications/verification.a_bounded_walk_yields_every_live_entry.patch`
/// The occupancy bound never hides a live entry. Fill three slots, remove an arbitrary subset
/// (all eight, holes under a live entry and the empty table included), and every walk must
/// report exactly as many entries as `len` claims. A subset rather than an operation sequence,
/// which did not finish in a practical time under Kani.
#[kani::proof]
fn a_bounded_walk_yields_every_live_entry() {
    let mut t: Table<u8, 3> = Table::new();
    let names = [
        t.insert_with(|_| 1).unwrap(),
        t.insert_with(|_| 2).unwrap(),
        t.insert_with(|_| 3).unwrap(),
    ];

    let drop_it: [bool; 3] = [kani::any(), kani::any(), kani::any()];
    for (name, drop_it) in names.iter().zip(drop_it) {
        if drop_it {
            t.remove(*name);
        }
    }

    assert_eq!(t.values().count(), t.len());
    assert_eq!(t.values_mut().count(), t.len());
    assert_eq!(t.live_slots().count(), t.len());
    assert_eq!(t.iter_from(0).count(), t.len());

    // And each survivor is still reachable by name, so the walk and the lookup agree.
    for (name, dropped) in names.iter().zip(drop_it) {
        assert_eq!(t.get(*name).is_some(), !dropped);
    }
}

/// Falsification: replayable `falsifications/verification.a_clone_resolves_exactly_the_same_names.patch`
/// A clone resolves exactly the names the original does: for any name at all, `get` agrees
/// between the two tables, so a live name reaches an equal entry and a dead or forged name
/// reaches nothing in both.
#[kani::proof]
fn a_clone_resolves_exactly_the_same_names() {
    let t = any_reachable_table();
    let copy = t.clone();
    let probe = EntryName::from_bits(kani::any());
    assert_eq!(t.get(probe), copy.get(probe));
    assert_eq!(t.len(), copy.len());
}
