//! This module provides extension to various types including, but not limited
//! to, hecs.

use hecs::{Component, Entity, EntityBuilder, EntityRef, World};

use crate::Name;

pub trait BuilderExt {
    /// Helper function for spawning entity builders
    fn spawn(&mut self, world: &mut World) -> Entity;
}

impl BuilderExt for EntityBuilder {
    fn spawn(&mut self, world: &mut World) -> Entity {
        world.spawn(self.build())
    }
}

pub struct WorldNameIterator<'a, 'w> {
    name: &'a Name,
    query: hecs::QueryIter<'w, &'static Name>,
}

impl<'a, 'w> Iterator for WorldNameIterator<'a, 'w> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((e, val)) = self.query.next() {
            if *val == *self.name {
                return Some(e);
            }
        }

        None
    }
}

pub struct WorldTagIterator<'w, T: Component> {
    query: hecs::QueryIter<'w, &'static T>,
}

impl<'w, T: Component> Iterator for WorldTagIterator<'w, T> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let (e, _) = self.query.next()?;
        Some(e)
    }
}

pub trait WorldExt {
    /// Finds an entity by name
    fn by_name(&self, name: Name) -> Option<EntityRef>;
    /// Finds an entity by tag
    fn by_tag<T: Component>(&self) -> Option<EntityRef>;
}

impl WorldExt for World {
    fn by_name(&self, name: Name) -> Option<EntityRef> {
        self.query::<&Name>()
            .iter()
            .find(|(_, val)| **val == name)
            .map(|(e, _)| self.entity(e).unwrap())
    }

    fn by_tag<T: Component>(&self) -> Option<EntityRef> {
        self.query::<&T>()
            .iter()
            .next()
            .map(|(e, _)| self.entity(e).unwrap())
    }
}
