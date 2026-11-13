//! Identities of the element tree: [`WidgetId`] lasts across builds, [`NodeId`] one build.

/// Stable identity of a widget across rebuilds, derived from its position in the
/// view tree (or from an explicit key, see [`BuildCx::keyed_child`](crate::BuildCx::keyed_child)). Hover,
/// press and focus are remembered per `WidgetId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct WidgetId(u64);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl WidgetId {
    #[doc(hidden)]
    pub const ROOT: Self = Self(0x5241_5645_4e52_5321);

    #[doc(hidden)]
    pub fn child(self, key: u64) -> Self {
        let mut h = self.0 ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Self(h ^ (h >> 31))
    }

    /// The raw identity, for derived ids such as accessibility node ids.
    #[doc(hidden)]
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// A node of the element tree produced by one build. Only valid during the
/// build that created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct NodeId(#[doc(hidden)] pub u32);
