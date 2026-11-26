//! The walks over a [`Table`], each a type a caller can name and store. Every walk visits the live
//! entries in slot order, skips free and retired slots, and stops at the table's walk bound, so it
//! costs what the table holds rather than `N`. Each walk but [`IterFromSlotIndex`] knows how many
//! entries it has left, so it implements [`ExactSizeIterator`]; every walk implements
//! [`FusedIterator`] and [`DoubleEndedIterator`]. A `for` loop takes a table by shared reference,
//! by mutable reference or by value, through the three [`IntoIterator`] implementations here.

use core::array;
use core::fmt;
use core::iter::FusedIterator;
use core::slice;

use crate::{EntryName, Table};

/// The occupied slots of a shared walk, from either end, each with its slot index and generation.
/// The two slice iterators always hold the same number of slots, and `front` is the slot index of
/// the next one from the front, so the next one from the back is `front` plus what is left.
struct LiveSlots<'a, T> {
    slots: slice::Iter<'a, Option<T>>,
    generations: slice::Iter<'a, u32>,
    front: usize,
}

impl<'a, T> LiveSlots<'a, T> {
    /// The slots `start..end` of `table`; `start` is at most `end`.
    fn new<const N: usize>(table: &'a Table<T, N>, start: usize, end: usize) -> Self {
        Self {
            slots: table.slots[start..end].iter(),
            generations: table.generations[start..end].iter(),
            front: start,
        }
    }

    fn next(&mut self) -> Option<(usize, u32, &'a T)> {
        loop {
            let entry = self.slots.next()?;
            let generation = *self.generations.next()?;
            let slot = self.front;
            self.front += 1;
            if let Some(value) = entry {
                return Some((slot, generation, value));
            }
        }
    }

    fn next_back(&mut self) -> Option<(usize, u32, &'a T)> {
        loop {
            let entry = self.slots.next_back()?;
            let generation = *self.generations.next_back()?;
            if let Some(value) = entry {
                return Some((self.front + self.slots.len(), generation, value));
            }
        }
    }

    /// How many slots, occupied or not, the walk has left.
    fn slots_left(&self) -> usize {
        self.slots.len()
    }
}

impl<T> Clone for LiveSlots<'_, T> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            generations: self.generations.clone(),
            front: self.front,
        }
    }
}

/// [`LiveSlots`], by mutable reference to each entry.
struct LiveSlotsMut<'a, T> {
    slots: slice::IterMut<'a, Option<T>>,
    generations: slice::Iter<'a, u32>,
    front: usize,
}

impl<'a, T> LiveSlotsMut<'a, T> {
    /// Every slot below the walk bound of `table`.
    fn new<const N: usize>(table: &'a mut Table<T, N>) -> Self {
        let top = table.top;
        Self {
            slots: table.slots[..top].iter_mut(),
            generations: table.generations[..top].iter(),
            front: 0,
        }
    }

    fn next(&mut self) -> Option<(usize, u32, &'a mut T)> {
        loop {
            let entry = self.slots.next()?;
            let generation = *self.generations.next()?;
            let slot = self.front;
            self.front += 1;
            if let Some(value) = entry {
                return Some((slot, generation, value));
            }
        }
    }

    fn next_back(&mut self) -> Option<(usize, u32, &'a mut T)> {
        loop {
            let entry = self.slots.next_back()?;
            let generation = *self.generations.next_back()?;
            if let Some(value) = entry {
                return Some((self.front + self.slots.len(), generation, value));
            }
        }
    }
}

/// Each live entry by shared reference, in slot order: what [`Table::values`] returns.
pub struct Values<'a, T> {
    live: LiveSlots<'a, T>,
    remaining: usize,
}

impl<'a, T> Values<'a, T> {
    pub(crate) fn new<const N: usize>(table: &'a Table<T, N>) -> Self {
        Self {
            live: LiveSlots::new(table, 0, table.top),
            remaining: table.live,
        }
    }
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.remaining == 0 {
            return None;
        }
        let (_, _, value) = self.live.next()?;
        self.remaining -= 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, T> DoubleEndedIterator for Values<'a, T> {
    fn next_back(&mut self) -> Option<&'a T> {
        if self.remaining == 0 {
            return None;
        }
        let (_, _, value) = self.live.next_back()?;
        self.remaining -= 1;
        Some(value)
    }
}

impl<T> ExactSizeIterator for Values<'_, T> {}

impl<T> FusedIterator for Values<'_, T> {}

impl<T> Clone for Values<'_, T> {
    fn clone(&self) -> Self {
        Self {
            live: self.live.clone(),
            remaining: self.remaining,
        }
    }
}

/// Prints the entries the walk has left, as a list.
impl<T: fmt::Debug> fmt::Debug for Values<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// Each live entry by mutable reference, in slot order: what [`Table::values_mut`] returns.
pub struct ValuesMut<'a, T> {
    live: LiveSlotsMut<'a, T>,
    remaining: usize,
}

impl<'a, T> ValuesMut<'a, T> {
    pub(crate) fn new<const N: usize>(table: &'a mut Table<T, N>) -> Self {
        let remaining = table.live;
        Self {
            live: LiveSlotsMut::new(table),
            remaining,
        }
    }
}

impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        if self.remaining == 0 {
            return None;
        }
        let (_, _, value) = self.live.next()?;
        self.remaining -= 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, T> DoubleEndedIterator for ValuesMut<'a, T> {
    fn next_back(&mut self) -> Option<&'a mut T> {
        if self.remaining == 0 {
            return None;
        }
        let (_, _, value) = self.live.next_back()?;
        self.remaining -= 1;
        Some(value)
    }
}

impl<T> ExactSizeIterator for ValuesMut<'_, T> {}

impl<T> FusedIterator for ValuesMut<'_, T> {}

/// Prints how many entries the walk has left, and nothing of the entries, which it holds mutably.
impl<T> fmt::Debug for ValuesMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValuesMut")
            .field("remaining", &self.remaining)
            .finish()
    }
}

/// Each live entry with its name, by shared reference, in slot order: what [`Table::iter`]
/// returns, and what a `for` loop over `&table` walks.
pub struct Iter<'a, T> {
    live: LiveSlots<'a, T>,
    remaining: usize,
}

impl<'a, T> Iter<'a, T> {
    pub(crate) fn new<const N: usize>(table: &'a Table<T, N>) -> Self {
        Self {
            live: LiveSlots::new(table, 0, table.top),
            remaining: table.live,
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (EntryName<T>, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let (slot, generation, value) = self.live.next()?;
        self.remaining -= 1;
        Some((EntryName::new(slot, generation), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let (slot, generation, value) = self.live.next_back()?;
        self.remaining -= 1;
        Some((EntryName::new(slot, generation), value))
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

impl<T> FusedIterator for Iter<'_, T> {}

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            live: self.live.clone(),
            remaining: self.remaining,
        }
    }
}

/// Prints the entries the walk has left, as a map from name to entry.
impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.clone()).finish()
    }
}

/// Each live entry with its name, by mutable reference, in slot order: what [`Table::iter_mut`]
/// returns, and what a `for` loop over `&mut table` walks.
pub struct IterMut<'a, T> {
    live: LiveSlotsMut<'a, T>,
    remaining: usize,
}

impl<'a, T> IterMut<'a, T> {
    pub(crate) fn new<const N: usize>(table: &'a mut Table<T, N>) -> Self {
        let remaining = table.live;
        Self {
            live: LiveSlotsMut::new(table),
            remaining,
        }
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (EntryName<T>, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let (slot, generation, value) = self.live.next()?;
        self.remaining -= 1;
        Some((EntryName::new(slot, generation), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> DoubleEndedIterator for IterMut<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let (slot, generation, value) = self.live.next_back()?;
        self.remaining -= 1;
        Some((EntryName::new(slot, generation), value))
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {}

impl<T> FusedIterator for IterMut<'_, T> {}

/// Prints how many entries the walk has left, and nothing of the entries, which it holds mutably.
impl<T> fmt::Debug for IterMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IterMut")
            .field("remaining", &self.remaining)
            .finish()
    }
}

/// Each live entry from a slot index onward, as `(slot index, name, &entry)`, in slot order:
/// what [`Table::iter_from_slot_index`] returns.
///
/// It is not an [`ExactSizeIterator`]: the table counts its live entries, not those above a
/// slot, so the exact length would cost a walk. [`size_hint`](Iterator::size_hint) gives a bound.
pub struct IterFromSlotIndex<'a, T> {
    live: LiveSlots<'a, T>,
    /// The table's live count, an upper bound on what the walk can yield.
    live_count: usize,
}

impl<'a, T> IterFromSlotIndex<'a, T> {
    pub(crate) fn new<const N: usize>(table: &'a Table<T, N>, first_slot_index: usize) -> Self {
        let start = first_slot_index.min(table.top);
        Self {
            live: LiveSlots::new(table, start, table.top),
            live_count: table.live,
        }
    }
}

impl<'a, T> Iterator for IterFromSlotIndex<'a, T> {
    type Item = (usize, EntryName<T>, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let (slot, generation, value) = self.live.next()?;
        self.live_count -= 1;
        Some((slot, EntryName::new(slot, generation), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.live.slots_left().min(self.live_count)))
    }
}

impl<T> DoubleEndedIterator for IterFromSlotIndex<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (slot, generation, value) = self.live.next_back()?;
        self.live_count -= 1;
        Some((slot, EntryName::new(slot, generation), value))
    }
}

impl<T> FusedIterator for IterFromSlotIndex<'_, T> {}

impl<T> Clone for IterFromSlotIndex<'_, T> {
    fn clone(&self) -> Self {
        Self {
            live: self.live.clone(),
            live_count: self.live_count,
        }
    }
}

/// Prints the entries the walk has left, as a list of `(slot index, name, entry)`.
impl<T: fmt::Debug> fmt::Debug for IterFromSlotIndex<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// Each live entry with its name, by value, in slot order: what a `for` loop over a table walks
/// when it takes the table itself. Entries the walk has not reached are dropped with it.
pub struct IntoIter<T, const N: usize> {
    slots: array::IntoIter<Option<T>, N>,
    generations: [u32; N],
    /// The slot index of the next slot from the front.
    front: usize,
    remaining: usize,
}

impl<T, const N: usize> IntoIter<T, N> {
    fn new(table: Table<T, N>) -> Self {
        let mut slots = table.slots.into_iter();
        // Drop the free slots above the walk bound now, so a walk from the back starts at the
        // highest live entry rather than at slot `N - 1`.
        if table.top < N {
            slots.nth_back(N - table.top - 1);
        }
        Self {
            slots,
            generations: table.generations,
            front: 0,
            remaining: table.live,
        }
    }
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = (EntryName<T>, T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        loop {
            let entry = self.slots.next()?;
            let slot = self.front;
            self.front += 1;
            if let Some(value) = entry {
                self.remaining -= 1;
                return Some((EntryName::new(slot, self.generations[slot]), value));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        loop {
            let entry = self.slots.next_back()?;
            if let Some(value) = entry {
                let slot = self.front + self.slots.len();
                self.remaining -= 1;
                return Some((EntryName::new(slot, self.generations[slot]), value));
            }
        }
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

/// Prints how many entries the walk has left.
impl<T, const N: usize> fmt::Debug for IntoIter<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoIter")
            .field("remaining", &self.remaining)
            .finish()
    }
}

/// `for (name, entry) in &table`, the same walk as [`Table::iter`].
impl<'a, T, const N: usize> IntoIterator for &'a Table<T, N> {
    type Item = (EntryName<T>, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

/// `for (name, entry) in &mut table`, the same walk as [`Table::iter_mut`].
impl<'a, T, const N: usize> IntoIterator for &'a mut Table<T, N> {
    type Item = (EntryName<T>, &'a mut T);
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

/// `for (name, entry) in table`, which takes the table and hands each entry over by value with
/// the name it had. The names die with the table.
impl<T, const N: usize> IntoIterator for Table<T, N> {
    type Item = (EntryName<T>, T);
    type IntoIter = IntoIter<T, N>;

    fn into_iter(self) -> IntoIter<T, N> {
        IntoIter::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{EntryName, Iter, Table};
    use std::rc::Rc;

    /// A table of four with a hole at slot 1: entries 10, 30 and 40 at slots 0, 2 and 3.
    fn table_with_a_hole() -> (Table<u32, 6>, [EntryName<u32>; 4]) {
        let mut t: Table<u32, 6> = Table::new();
        let names = [10, 20, 30, 40].map(|v| t.insert(v).unwrap());
        t.remove(names[1]);
        (t, names)
    }

    /// Every walk counts down exactly, from the front and from the back alike.
    #[test]
    fn each_walk_reports_its_exact_length_as_it_goes() {
        let (mut t, _) = table_with_a_hole();
        let mut values = t.values();
        assert_eq!(values.len(), 3);
        values.next();
        assert_eq!(values.size_hint(), (2, Some(2)));
        values.next_back();
        assert_eq!(values.len(), 1);
        assert_eq!(values.next(), Some(&30));
        assert_eq!(values.len(), 0);

        assert_eq!(t.iter().len(), 3);
        assert_eq!(t.iter_mut().len(), 3);
        let mut values_mut = t.values_mut();
        values_mut.next_back();
        assert_eq!(values_mut.len(), 2);
    }

    /// Steps a walk of three from the front, the back and the front again, checking after each
    /// step that `len` and `size_hint` both say exactly what is left.
    fn counts_down_exactly<I: DoubleEndedIterator + ExactSizeIterator>(mut walk: I) {
        assert_eq!((walk.len(), walk.size_hint()), (3, (3, Some(3))));
        assert!(walk.next().is_some());
        assert_eq!((walk.len(), walk.size_hint()), (2, (2, Some(2))));
        assert!(walk.next_back().is_some());
        assert_eq!((walk.len(), walk.size_hint()), (1, (1, Some(1))));
        assert!(walk.next().is_some());
        assert_eq!((walk.len(), walk.size_hint()), (0, (0, Some(0))));
        assert!(walk.next().is_none() && walk.next_back().is_none());
    }

    /// Every walk with an exact length keeps it exact after each step from either end.
    #[test]
    fn every_exact_walk_counts_down_after_each_step() {
        let (mut t, _) = table_with_a_hole();
        counts_down_exactly(t.values());
        counts_down_exactly(t.values_mut());
        counts_down_exactly(t.iter());
        counts_down_exactly(t.iter_mut());
        counts_down_exactly(t.clone().into_iter());
        counts_down_exactly(t.iter().rev());
        counts_down_exactly(t.values_mut().rev());
        counts_down_exactly(t.iter_mut().rev());
        counts_down_exactly(t.into_iter().rev());
    }

    /// The bound a walk from a slot index gives drops with each entry it yields, from either
    /// end, when the live count is the tighter of its two bounds.
    #[test]
    fn a_walk_from_a_slot_index_lowers_its_bound_after_each_step() {
        let (t, _) = table_with_a_hole();
        let mut from = t.iter_from_slot_index(0);
        assert_eq!(from.size_hint(), (0, Some(3)));
        assert_eq!(from.next().map(|entry| entry.0), Some(0));
        assert_eq!(
            from.size_hint(),
            (0, Some(2)),
            "two live entries left in three slots"
        );
        assert_eq!(from.next_back().map(|entry| entry.0), Some(3));
        assert_eq!(
            from.size_hint(),
            (0, Some(1)),
            "one live entry left in two slots"
        );
        assert_eq!(from.next().map(|entry| entry.0), Some(2));
        assert_eq!(from.size_hint(), (0, Some(0)));

        let mut from = t.iter_from_slot_index(0);
        assert_eq!(from.next_back().map(|entry| entry.0), Some(3));
        assert_eq!(
            from.size_hint(),
            (0, Some(2)),
            "two live entries left in three slots"
        );
    }

    /// From the back, each walk yields the same entries under the same names, in reverse.
    #[test]
    fn each_walk_runs_backward_under_the_same_names() {
        let (mut t, names) = table_with_a_hole();
        let forward: Vec<_> = t.iter().map(|(n, v)| (n, *v)).collect();
        let mut backward: Vec<_> = t.iter().rev().map(|(n, v)| (n, *v)).collect();
        backward.reverse();
        assert_eq!(forward, backward);
        assert_eq!(
            forward,
            vec![(names[0], 10), (names[2], 30), (names[3], 40)]
        );

        assert_eq!(t.values().rev().copied().collect::<Vec<_>>(), [40, 30, 10]);
        let mut mutable_backward = Vec::new();
        for (name, value) in t.iter_mut().rev() {
            *value += 1;
            mutable_backward.push(name);
        }
        assert_eq!(mutable_backward, [names[3], names[2], names[0]]);
        for value in t.values_mut().rev() {
            *value += 1;
        }
        assert_eq!(t.values().copied().collect::<Vec<_>>(), [12, 32, 42]);
        assert_eq!(
            t.iter_from_slot_index(1)
                .rev()
                .map(|(slot, name, _)| (slot, name))
                .collect::<Vec<_>>(),
            [(3, names[3]), (2, names[2])]
        );
    }

    /// The two ends meet in the middle and neither yields an entry twice.
    #[test]
    fn the_two_ends_meet_without_a_repeat() {
        let (t, _) = table_with_a_hole();
        let mut iter = t.iter_from_slot_index(0);
        assert_eq!(iter.next().map(|(slot, _, _)| slot), Some(0));
        assert_eq!(iter.next_back().map(|(slot, _, _)| slot), Some(3));
        assert_eq!(iter.next_back().map(|(slot, _, _)| slot), Some(2));
        assert!(iter.next().is_none() && iter.next_back().is_none());
    }

    /// Once a walk ends it stays ended.
    #[test]
    fn a_finished_walk_stays_finished() {
        let (mut t, _) = table_with_a_hole();
        let mut iter = t.iter();
        iter.by_ref().for_each(drop);
        assert!(iter.next().is_none() && iter.next_back().is_none());
        let mut from = t.iter_from_slot_index(3);
        assert!(from.next().is_some());
        assert!(from.next().is_none() && from.next().is_none());
        let mut values_mut = t.values_mut();
        values_mut.by_ref().for_each(drop);
        assert!(values_mut.next().is_none());
    }

    /// A walk from a slot index bounds its length by the slots it has left and by the live count,
    /// and yields nothing from past the walk bound.
    #[test]
    fn a_walk_from_a_slot_index_bounds_its_length() {
        let (t, _) = table_with_a_hole();
        assert_eq!(t.iter_from_slot_index(0).size_hint(), (0, Some(3)));
        assert_eq!(t.iter_from_slot_index(2).size_hint(), (0, Some(2)));
        let mut from = t.iter_from_slot_index(2);
        from.next();
        assert_eq!(from.size_hint(), (0, Some(1)));
        assert_eq!(t.iter_from_slot_index(4).size_hint(), (0, Some(0)));
        assert_eq!(t.iter_from_slot_index(usize::MAX).count(), 0);
    }

    /// A caller can name a walk's type and keep the walk in a struct.
    #[test]
    fn a_walk_can_be_kept_in_a_field() {
        struct Sweep<'a> {
            entries: Iter<'a, u32>,
        }
        let (t, names) = table_with_a_hole();
        let mut sweep = Sweep { entries: t.iter() };
        let copy = sweep.entries.clone();
        assert_eq!(sweep.entries.next(), Some((names[0], &10)));
        assert_eq!(copy.count(), 3, "a clone walks on its own");
        assert_eq!(t.values().clone().count(), 3);
    }

    /// The shared walks print what they have left; the mutable ones print only how much.
    #[test]
    fn each_walk_prints_what_it_has_left() {
        let (mut t, _) = table_with_a_hole();
        let mut values = t.values();
        values.next();
        assert_eq!(format!("{values:?}"), "[30, 40]");
        let mut iter = t.iter();
        iter.next();
        iter.next();
        assert_eq!(
            format!("{iter:?}"),
            "{EntryName { generation: 0, slot: 3 }: 40}"
        );
        assert_eq!(
            format!("{:?}", t.iter_from_slot_index(3)),
            "[(3, EntryName { generation: 0, slot: 3 }, 40)]"
        );
        assert_eq!(
            format!("{:?}", t.values_mut()),
            "ValuesMut { remaining: 3 }"
        );
        assert_eq!(format!("{:?}", t.iter_mut()), "IterMut { remaining: 3 }");
    }

    /// A `for` loop takes a table by shared reference, by mutable reference and by value, and
    /// each yields the names `iter` yields.
    #[test]
    fn a_for_loop_walks_a_table_three_ways() {
        let (mut t, names) = table_with_a_hole();
        let expected = [(names[0], 10), (names[2], 30), (names[3], 40)];

        let mut shared = Vec::new();
        for (name, value) in &t {
            shared.push((name, *value));
        }
        assert_eq!(shared, expected);

        for (_, value) in &mut t {
            *value += 1;
        }
        let owned: Vec<_> = t.into_iter().collect();
        assert_eq!(owned, expected.map(|(n, v)| (n, v + 1)));
    }

    /// The walk by value runs from either end, knows its length, and stays finished.
    #[test]
    fn the_walk_by_value_runs_from_either_end() {
        let (t, names) = table_with_a_hole();
        let mut owned = t.into_iter();
        assert_eq!(owned.len(), 3);
        assert_eq!(owned.next_back(), Some((names[3], 40)));
        assert_eq!(owned.next(), Some((names[0], 10)));
        assert_eq!(format!("{owned:?}"), "IntoIter { remaining: 1 }");
        assert_eq!(owned.next_back(), Some((names[2], 30)));
        assert_eq!(owned.size_hint(), (0, Some(0)));
        assert!(owned.next().is_none() && owned.next_back().is_none());

        let full: Table<u8, 2> = {
            let mut t = Table::new();
            t.insert(1).unwrap();
            t.insert(2).unwrap();
            t
        };
        assert_eq!(
            full.into_iter().rev().map(|(_, v)| v).collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(Table::<u8, 3>::new().into_iter().count(), 0);
    }

    /// Entries the walk by value has not reached are dropped with it, once each.
    #[test]
    fn the_walk_by_value_drops_what_it_leaves() {
        let witness = Rc::new(());
        let mut t: Table<Rc<()>, 4> = Table::new();
        for _ in 0..3 {
            t.insert(Rc::clone(&witness)).unwrap();
        }
        let mut owned = t.into_iter();
        let (_, first) = owned.next().unwrap();
        assert_eq!(Rc::strong_count(&witness), 4);
        drop(owned);
        assert_eq!(Rc::strong_count(&witness), 2, "the walk kept what it left");
        drop(first);
        assert_eq!(Rc::strong_count(&witness), 1);
    }
}
