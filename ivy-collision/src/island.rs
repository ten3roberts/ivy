use core::fmt;
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Formatter,
};

use itertools::Itertools;
use slotmap::{Key, SecondaryMap, SlotMap};

use crate::{
    body::{Body, BodyIndex, BodyMap, ContactIndex, ContactMap},
    util::IndexedRange,
    Contact,
};

use crate::tree::NodeIndex;

pub type Nodes<N> = SlotMap<NodeIndex, N>;

pub struct IslandContactIter<'a> {
    contacts: &'a ContactMap,
    index: ContactIndex,
    head: ContactIndex,
}

impl<'a> Iterator for IslandContactIter<'a> {
    type Item = (ContactIndex, &'a Contact);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        if index.is_null() {
            return None;
        }

        let contact = &self.contacts[index];
        self.index = contact.next_contact;
        assert_ne!(self.index, self.head, "circular contact list");
        Some((index, contact))
    }
}

#[derive(Debug, Clone)]
pub struct Island {
    // parent or self
    parent: BodyIndex,
    head_body: BodyIndex,
    // used to rebuild island graph components during split
    head_contact: ContactIndex,
}

impl Island {
    fn add_contact(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) {
        let contact = &mut contacts[contact_index];
        // assert!(contact.island.is_null());
        assert!(
            contact.next_contact.is_null(),
            "contact is already connected"
        );

        contact.next_contact = self.head_contact;
        contact.prev_contact = ContactIndex::null();

        if !self.head_contact.is_null() {
            contacts[self.head_contact].prev_contact = contact_index;
        }

        self.head_contact = contact_index;
    }

    fn remove_contact(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) {
        let contact = &mut contacts[contact_index];
        contact.island = BodyIndex::null();

        if contact_index == self.head_contact {
            let next_index = contact.next_contact;
            contact.next_contact = ContactIndex::null();
            assert!(contact.prev_contact.is_null());

            self.head_contact = next_index;

            if !next_index.is_null() {
                let next = &mut contacts[next_index];
                assert_eq!(next.prev_contact, contact_index, "next: {next_index:?}");
                next.prev_contact = ContactIndex::null();
            }
        } else {
            let prev = contact.prev_contact;
            assert!(!prev.is_null());
            let next = contact.next_contact;

            assert_ne!(next, prev);
            assert_eq!(contacts[prev].next_contact, contact_index);

            contacts[prev].next_contact = next;
            if !next.is_null() {
                assert_eq!(contacts[next].prev_contact, contact_index);
                contacts[next].prev_contact = prev;
            }
        }
    }

    fn add_body(&mut self, bodies: &mut BodyMap, body_index: BodyIndex) {
        let body = &mut bodies[body_index];
        assert!(body.next_body.is_null());

        body.next_body = self.head_body;

        if !self.head_body.is_null() {
            bodies[self.head_body].prev_body = body_index;
        }

        self.head_body = body_index;
        assert!(!self.head_body.is_null());
    }

    pub fn contacts<'a>(&self, contacts: &'a ContactMap) -> IslandContactIter<'a> {
        IslandContactIter {
            head: self.head_contact,
            contacts,
            index: self.head_contact,
        }
    }

    pub fn parent(&self) -> BodyIndex {
        self.parent
    }

    fn from_body(body_index: BodyIndex) -> Self {
        Self {
            parent: body_index,
            head_body: BodyIndex::null(),
            head_contact: ContactIndex::null(),
        }
    }
}

#[derive(Default)]
struct Scratch {
    bodies: Vec<BodyIndex>,
    visited_bodies: SecondaryMap<BodyIndex, ()>,
    visited_contacts: SecondaryMap<ContactIndex, ()>,
}

pub struct Islands {
    islands: SecondaryMap<BodyIndex, Island>,
    static_set: SecondaryMap<BodyIndex, ()>,
    scratch: Scratch,
}

impl std::fmt::Debug for Islands {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut s = f.debug_map();

        for (k, v) in &self.islands {
            s.entry(&format!("{k:?}"), &v);
        }

        s.finish()
    }
}

impl Islands {
    pub fn new() -> Self {
        Self {
            islands: Default::default(),
            static_set: Default::default(),
            scratch: Default::default(),
        }
    }

    pub fn create_island(&mut self, body_index: BodyIndex) {
        let island = Island {
            parent: body_index,
            head_body: BodyIndex::null(),
            head_contact: ContactIndex::null(),
        };

        self.islands.insert(body_index, island);
    }

    pub fn representative_compress(&mut self, index: BodyIndex) -> Option<BodyIndex> {
        let mut index = index;
        if self.static_set.contains_key(index) {
            return None;
        }

        loop {
            let node = &mut self.islands[index];
            let parent = node.parent;

            if parent == index {
                break;
            }

            let next_parent = self.islands[parent].parent;
            self.islands[index].parent = next_parent;

            index = parent;
        }

        Some(index)
    }

    pub fn link(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) -> &mut Island {
        let contact = &mut contacts[contact_index];
        let a = contact.a.body;
        let b = contact.b.body;

        let a_rep = self.representative_compress(a);
        let b_rep = self.representative_compress(b);

        match (a_rep, b_rep) {
            (None, None) => {
                panic!("static bodies can not collide");
            }
            (None, Some(b_rep)) => {
                contact.island = b_rep;
                let island = &mut self.islands[b_rep];
                island.add_contact(contacts, contact_index);
                island
            }
            (Some(a_rep), None) => {
                contact.island = a_rep;
                let island = &mut self.islands[a_rep];
                island.add_contact(contacts, contact_index);
                island
            }
            (Some(a_rep), Some(b_rep)) if a_rep == b_rep => {
                contact.island = a_rep;
                let island = &mut self.islands[a_rep];

                island.add_contact(contacts, contact_index);
                island
            }
            (Some(a_rep), Some(b_rep)) => {
                contact.island = a_rep;
                self.islands[b_rep].parent = a_rep;

                let island = &mut self.islands[a_rep];

                island.add_contact(contacts, contact_index);
                island
            }
        }
    }

    // Unlink a contact from it's island
    //
    // Does not split the island
    pub fn unlink(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) {
        let contact = &mut contacts[contact_index];

        let island_index = contact.island;
        contact.island = BodyIndex::null();
        let island = &mut self.islands[island_index];
        island.remove_contact(contacts, contact_index);
    }

    pub fn verify_depth(&self) {
        for (island_index, island) in &self.islands {
            if !island.head_body.is_null() {
                assert_eq!(island.parent, island_index);
            }
        }
    }

    pub fn verify(&self, bodies: &BodyMap, contacts: &ContactMap) {
        let mut found = HashSet::new();
        for (island_index, island) in &self.islands {
            let mut prev = ContactIndex::null();
            let mut contact_index = island.head_contact;

            while !contact_index.is_null() {
                assert!(!found.contains(&contact_index));
                found.insert(contact_index);

                let contact = &contacts[contact_index];
                assert_eq!(contact.island, island_index);
                assert_eq!(contact.prev_contact, prev);

                prev = contact_index;
                contact_index = contact.next_contact;
            }

            let mut body_index = island.head_body;
            while !body_index.is_null() {
                let body = &bodies[body_index];
                let subisland = &self.islands[body_index];
                assert_eq!(subisland.parent, island_index);
                body_index = body.next_body;
            }
        }

        for (k, v) in contacts {
            let prev = v.prev_contact;
            let next = v.next_contact;

            assert!(prev.is_null() || contacts[prev].next_contact == k);
            assert!(next.is_null() || contacts[next].prev_contact == k);
        }

        for contact in contacts.keys() {
            assert!(found.contains(&contact), "contact {contact:?} missing");
        }
    }

    /// merges all island bodies into the roots
    pub fn merge_root_islands(&mut self, contacts: &mut ContactMap, bodies: &mut BodyMap) {
        let keys = self.islands.keys().collect_vec();

        for index in keys {
            let Some(parent_index) = self.representative_compress(index) else {
                continue;
            };

            if parent_index == index {
                continue;
            }

            let [island, parent] = self
                .islands
                .get_disjoint_mut([index, parent_index])
                .unwrap();

            island.parent = parent_index;

            let mut contact_index = island.head_contact;
            while !contact_index.is_null() {
                let contact = &mut contacts[contact_index];
                contact.island = parent_index;

                let next = contact.next_contact;
                // reached end, attach parent list
                if next.is_null() {
                    contact.next_contact = parent.head_contact;
                    if !parent.head_contact.is_null() {
                        assert!(contacts[parent.head_contact].prev_contact.is_null());
                        contacts[parent.head_contact].prev_contact = contact_index;
                    }

                    parent.head_contact = island.head_contact;
                }

                contact_index = next;
            }
            island.head_contact = ContactIndex::null();

            let mut body_index = island.head_body;
            while !body_index.is_null() {
                let body = &mut bodies[body_index];
                // assert_eq!(body.island, index);
                body.island = parent_index;

                let next = body.next_body;
                // reached end, attach parent list
                if next.is_null() {
                    body.next_body = parent.head_body;
                    if !parent.head_body.is_null() {
                        assert!(bodies[parent.head_body].prev_body.is_null());
                        bodies[parent.head_body].prev_body = body_index;
                    }

                    parent.head_body = island.head_body;
                    assert!(!island.head_body.is_null());
                }

                body_index = next;
            }

            island.head_body = BodyIndex::null();
        }
    }

    pub fn reconstruct(
        &mut self,
        island_index: BodyIndex,
        bodies: &mut BodyMap,
        contacts: &mut ContactMap,
        contact_map: &BTreeMap<(BodyIndex, IndexedRange<BodyIndex>), ContactIndex>,
    ) {
        // let _span = tracing::info_span!("reconstruct", ?island_index).entered();
        let island = &self.islands[island_index];

        let mut all_contacts = Vec::new();

        // tracing::info!("{self:#?}");
        // tracing::info!("{}", contacts.iter().map(|v| format!("{v:?}")).join("\n"));

        {
            let mut contact_index = island.head_contact;
            while !contact_index.is_null() {
                let contact = &mut contacts[contact_index];

                let next = contact.next_contact;
                contact.island = BodyIndex::null();
                contact.next_contact = ContactIndex::null();
                contact.prev_contact = ContactIndex::null();

                all_contacts.push(contact_index);
                contact_index = next;
            }
        }

        let all_bodies = &mut self.scratch.bodies;
        all_bodies.clear();
        {
            let mut body_index = island.head_body;
            while !body_index.is_null() {
                assert!(!self.static_set.contains_key(body_index));

                let body = &mut bodies[body_index];
                let next = body.next_body;

                body.next_body = BodyIndex::null();
                body.prev_body = BodyIndex::null();

                self.islands[body_index] = Island::from_body(body_index);

                all_bodies.push(body_index);
                body_index = next;
            }
        }

        // tracing::info!(?all_bodies, ?all_contacts);

        assert!(!all_bodies.is_empty());

        let visited = &mut self.scratch.visited_bodies;
        visited.clear();

        let visited_contacts = &mut self.scratch.visited_contacts;
        visited_contacts.clear();

        for &body_index in all_bodies.iter() {
            // found in connection from another seed
            if visited.contains_key(body_index) {
                continue;
            }

            let seed_index = body_index;

            let _span = tracing::info_span!("seed", ?seed_index).entered();

            let mut stack = vec![body_index];
            while let Some(body_index) = stack.pop() {
                assert!(all_bodies.contains(&body_index));
                if visited.contains_key(body_index) {
                    continue;
                }

                visited.insert(body_index, ());

                let island = &mut self.islands[body_index];
                island.parent = seed_index;

                let body = &mut bodies[body_index];
                body.island = seed_index;
                body.next_body = BodyIndex::null();
                body.prev_body = BodyIndex::null();

                let seed_island = &mut self.islands[seed_index];
                seed_island.add_body(bodies, body_index);

                let edges = contact_map
                    .range((body_index, IndexedRange::Min)..(body_index, IndexedRange::Max));

                for ((_, other_index), &contact_index) in edges {
                    let &other_index = other_index.as_exact().unwrap();
                    assert!(
                        self.static_set.contains_key(other_index)
                            || all_bodies.contains(&other_index)
                    );

                    if visited_contacts.contains_key(contact_index) {
                        continue;
                    }

                    visited_contacts.insert(contact_index, ());

                    // connect contact to this island
                    let contact = &mut contacts[contact_index];

                    // tracing::info!(
                    //     ?contact_index,
                    //     ?contact,
                    //     "{:?} {:?}",
                    //     bodies[contact.a.body].state,
                    //     bodies[contact.b.body].state
                    // );
                    assert_eq!(contact.island, BodyIndex::null());
                    contact.island = seed_index;
                    seed_island.add_contact(contacts, contact_index);

                    if !self.static_set.contains_key(other_index)
                        && !visited.contains_key(other_index)
                    {
                        stack.push(other_index);
                    }
                }
            }
        }

        for contact_index in all_contacts {
            assert_ne!(
                contacts[contact_index].island,
                BodyIndex::null(),
                "contact_map: {:?}",
                contact_map.iter().collect_vec()
            );
        }
    }

    pub fn link_body(&mut self, bodies: &mut SlotMap<BodyIndex, Body>, index: BodyIndex) {
        self.islands[index].add_body(bodies, index)
    }

    pub fn mark_static(&mut self, index: BodyIndex) {
        self.static_set.insert(index, ());
    }

    pub fn islands(&self) -> &SecondaryMap<BodyIndex, Island> {
        &self.islands
    }

    pub fn static_set(&self) -> &SecondaryMap<BodyIndex, ()> {
        &self.static_set
    }
}
