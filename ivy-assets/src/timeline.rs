use std::{any::TypeId, collections::BTreeMap, time::Instant};

use slab::Slab;

use crate::AssetId;

#[derive(Clone)]
pub struct AssetInfo {
    pub name: String,
    pub asset_type: TypeId,
    pub type_name: String,
}

#[derive(Clone)]
pub struct AssetTimeSpan {
    info: AssetInfo,
    asset: Option<AssetId>,
    parent: Option<usize>,
    load_start: Instant,
    load_end: Option<Instant>,
}

impl AssetTimeSpan {
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }

    pub fn load_start(&self) -> Instant {
        self.load_start
    }

    pub fn load_end(&self) -> Option<Instant> {
        self.load_end
    }

    pub fn asset(&self) -> Option<AssetId> {
        self.asset
    }

    pub fn info(&self) -> &AssetInfo {
        &self.info
    }
}

#[derive(Clone)]
pub struct Timelines {
    spans: Slab<AssetTimeSpan>,
    edge_map: BTreeMap<usize, Vec<usize>>,
    roots: Vec<usize>,
}

impl Timelines {
    pub fn new() -> Self {
        Self {
            spans: Slab::new(),
            edge_map: Default::default(),
            roots: Default::default(),
        }
    }

    pub fn open_span(&mut self, info: AssetInfo, parent: Option<usize>) -> usize {
        let id = self.spans.insert(AssetTimeSpan {
            asset: None,
            parent,
            load_start: Instant::now(),
            load_end: None,
            info,
        });

        if let Some(parent) = parent {
            self.edge_map.entry(parent).or_default().push(id);
        } else {
            self.roots.push(id);
        }

        id
    }

    pub fn close_span(&mut self, id: usize, asset: Option<AssetId>) {
        let span = self.spans.get_mut(id).expect("unknown span");
        span.load_end.get_or_insert(Instant::now());

        span.asset = asset;
    }

    pub fn spans(&self) -> &Slab<AssetTimeSpan> {
        &self.spans
    }

    pub fn edge_map(&self) -> &BTreeMap<usize, Vec<usize>> {
        &self.edge_map
    }

    pub fn roots(&self) -> &[usize] {
        &self.roots
    }
}

impl Default for Timelines {
    fn default() -> Self {
        Self::new()
    }
}
