//! Contains representations of data from the adblocking engine in a
//! forwards-and-backwards-compatible format, as well as utilities for converting these to and from
//! the actual `Engine` components.
//!
//! Any new fields should be added to the _end_ of both `SerializeFormat` and `DeserializeFormat`.

use std::collections::{HashSet, HashMap};

use serde::{Deserialize, Serialize};
use rmp_serde as rmps;

use crate::blocker::{Blocker, NetworkFilterList};
use crate::resources::{RedirectResourceStorage, ScriptletResourceStorage};
use crate::filters::network::NetworkFilter;
use crate::cosmetic_filter_cache::{CosmeticFilterCache, HostnameRuleDb};

use super::{DeserializationError, SerializationError};
use super::utils::{stabilize_hashmap_serialization, stabilize_hashset_serialization};

/// `_bug` is no longer used, and is removed from future format versions.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkFilterV0SerializeFmt<'a> {
    mask: &'a crate::filters::network::NetworkFilterMask,
    filter: &'a crate::filters::network::FilterPart,
    opt_domains: &'a Option<Vec<crate::utils::Hash>>,
    opt_not_domains: &'a Option<Vec<crate::utils::Hash>>,
    redirect: &'a Option<String>,
    hostname: &'a Option<String>,
    csp: &'a Option<String>,
    _bug: Option<u32>,
    tag: &'a Option<String>,
    raw_line: Option<String>,
    id: &'a crate::utils::Hash,
    opt_domains_union: &'a Option<crate::utils::Hash>,
    opt_not_domains_union: &'a Option<crate::utils::Hash>,
}

/// Generic over `Borrow<NetworkFilter>` because `tagged_filters_all` requires `&'a NetworkFilter`
/// while `NetworkFilterList` requires `&'a Arc<NetworkFilter>`.
impl<'a, T> From<&'a T> for NetworkFilterV0SerializeFmt<'a> where T: std::borrow::Borrow<NetworkFilter> {
    fn from(v: &'a T) -> NetworkFilterV0SerializeFmt<'a> {
        let v = v.borrow();
        NetworkFilterV0SerializeFmt {
            mask: &v.mask,
            filter: &v.filter,
            opt_domains: &v.opt_domains,
            opt_not_domains: &v.opt_not_domains,
            redirect: if v.is_redirect() { &v.modifier_option } else { &None },
            hostname: &v.hostname,
            csp: if v.is_csp() { &v.modifier_option } else { &None },
            _bug: None,
            tag: &v.tag,
            raw_line: v.raw_line.as_ref().map(|raw| *raw.clone()),
            id: &v.id,
            opt_domains_union: &v.opt_domains_union,
            opt_not_domains_union: &v.opt_not_domains_union,
        }
    }
}

/// Forces a `NetworkFilterList` to be serialized with the v0 filter format by converting to an
/// intermediate representation that is constructed with `NetworkFilterV0Fmt` instead.
fn serialize_v0_network_filter_list<S>(list: &NetworkFilterList, s: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
    #[derive(Serialize, Default)]
    struct NetworkFilterListV0SerializeFmt<'a> {
        #[serde(serialize_with = "crate::data_format::utils::stabilize_hashmap_serialization")]
        filter_map: HashMap<crate::utils::Hash, Vec<NetworkFilterV0SerializeFmt<'a>>>,
    }

    let v0_list = NetworkFilterListV0SerializeFmt {
        filter_map: list.filter_map.iter().map(|(k, v)| {
            (*k, v.iter().map(|f| f.into()).collect())
        }).collect(),
    };

    v0_list.serialize(s)
}

/// Forces a `NetworkFilter` slice to be serialized with the v0 filter format by converting to
/// an intermediate representation that is constructed with `NetworkFilterV0Fmt` instead.
fn serialize_v0_network_filter_vec<S>(vec: &[NetworkFilter], s: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
    let v0_vec: Vec<_> = vec.iter().map(NetworkFilterV0SerializeFmt::from).collect();

    v0_vec.serialize(s)
}

/// Provides structural aggregration of referenced adblock engine data to allow for allocation-free
/// serialization.
#[derive(Serialize)]
pub(crate) struct SerializeFormat<'a> {
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    csp: &'a NetworkFilterList,
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    exceptions: &'a NetworkFilterList,
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    importants: &'a NetworkFilterList,
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    redirects: &'a NetworkFilterList,
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    filters_tagged: &'a NetworkFilterList,
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    filters: &'a NetworkFilterList,
    #[serde(serialize_with = "serialize_v0_network_filter_list")]
    generic_hide: &'a NetworkFilterList,

    #[serde(serialize_with = "serialize_v0_network_filter_vec")]
    tagged_filters_all: &'a Vec<NetworkFilter>,

    enable_optimizations: bool,

    resources: &'a RedirectResourceStorage,

    #[serde(serialize_with = "stabilize_hashset_serialization")]
    simple_class_rules: &'a HashSet<String>,
    #[serde(serialize_with = "stabilize_hashset_serialization")]
    simple_id_rules: &'a HashSet<String>,
    #[serde(serialize_with = "stabilize_hashmap_serialization")]
    complex_class_rules: &'a HashMap<String, Vec<String>>,
    #[serde(serialize_with = "stabilize_hashmap_serialization")]
    complex_id_rules: &'a HashMap<String, Vec<String>>,

    specific_rules: &'a HostnameRuleDb,

    #[serde(serialize_with = "stabilize_hashset_serialization")]
    misc_generic_selectors: &'a HashSet<String>,

    scriptlets: &'a ScriptletResourceStorage,
}

impl<'a> SerializeFormat<'a> {
    pub fn serialize(&self) -> Result<Vec<u8>, SerializationError> {
        let mut output = super::ADBLOCK_RUST_DAT_MAGIC.to_vec();
        output.push(0);
        rmps::encode::write(&mut output, &self)?;
        Ok(output)
    }
}

/// `_bug` is no longer used, and is cleaned up from future format versions.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct NetworkFilterV0DeserializeFmt {
    pub mask: crate::filters::network::NetworkFilterMask,
    pub filter: crate::filters::network::FilterPart,
    pub opt_domains: Option<Vec<crate::utils::Hash>>,
    pub opt_not_domains: Option<Vec<crate::utils::Hash>>,
    pub redirect: Option<String>,
    pub hostname: Option<String>,
    pub csp: Option<String>,
    _bug: Option<u32>,
    pub tag: Option<String>,
    pub raw_line: Option<String>,
    pub id: crate::utils::Hash,
    pub opt_domains_union: Option<crate::utils::Hash>,
    pub opt_not_domains_union: Option<crate::utils::Hash>,
}

impl From<NetworkFilterV0DeserializeFmt> for NetworkFilter {
    fn from(v: NetworkFilterV0DeserializeFmt) -> Self {
        Self {
            mask: v.mask,
            filter: v.filter,
            opt_domains: v.opt_domains,
            opt_not_domains: v.opt_not_domains,
            modifier_option: v.redirect.or(v.csp),
            hostname: v.hostname,
            tag: v.tag,
            raw_line: v.raw_line.map(Box::new),
            id: v.id,
            opt_domains_union: v.opt_domains_union,
            opt_not_domains_union: v.opt_not_domains_union,
            regex: crate::filters::network::RegexStorage::default(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct NetworkFilterListV0DeserializeFmt {
    pub filter_map: HashMap<crate::utils::Hash, Vec<NetworkFilterV0DeserializeFmt>>,
}

impl From<NetworkFilterListV0DeserializeFmt> for NetworkFilterList {
    fn from(v: NetworkFilterListV0DeserializeFmt) -> Self {
        Self {
            filter_map: v.filter_map.into_iter().map(|(k, v)| (k, v.into_iter().map(|f| std::sync::Arc::new(f.into())).collect())).collect(),
        }
    }
}

/// Structural representation of adblock engine data that can be built up from deserialization and
/// used directly to construct new `Engine` components without unnecessary allocation.
#[derive(Deserialize)]
pub(crate) struct DeserializeFormat {
    csp: NetworkFilterListV0DeserializeFmt,
    exceptions: NetworkFilterListV0DeserializeFmt,
    importants: NetworkFilterListV0DeserializeFmt,
    redirects: NetworkFilterListV0DeserializeFmt,
    filters_tagged: NetworkFilterListV0DeserializeFmt,
    filters: NetworkFilterListV0DeserializeFmt,
    generic_hide: NetworkFilterListV0DeserializeFmt,

    tagged_filters_all: Vec<NetworkFilterV0DeserializeFmt>,

    enable_optimizations: bool,

    resources: RedirectResourceStorage,

    simple_class_rules: HashSet<String>,
    simple_id_rules: HashSet<String>,
    complex_class_rules: HashMap<String, Vec<String>>,
    complex_id_rules: HashMap<String, Vec<String>>,

    specific_rules: HostnameRuleDb,

    misc_generic_selectors: HashSet<String>,

    scriptlets: ScriptletResourceStorage,
}

impl DeserializeFormat {
    pub fn deserialize(serialized: &[u8]) -> Result<Self, DeserializationError> {
        assert!(serialized.starts_with(&super::ADBLOCK_RUST_DAT_MAGIC));
        assert!(serialized[super::ADBLOCK_RUST_DAT_MAGIC.len()] == 0);
        let format: Self = rmps::decode::from_read(&serialized[super::ADBLOCK_RUST_DAT_MAGIC.len() + 1..])?;
        Ok(format)
    }
}

impl<'a> From<(&'a Blocker, &'a CosmeticFilterCache)> for SerializeFormat<'a> {
    fn from(v: (&'a Blocker, &'a CosmeticFilterCache)) -> Self {
        let (blocker, cfc) = v;
        Self {
            csp: &blocker.csp,
            exceptions: &blocker.exceptions,
            importants: &blocker.importants,
            redirects: &blocker.redirects,
            filters_tagged: &blocker.filters_tagged,
            filters: &blocker.filters,
            generic_hide: &blocker.generic_hide,

            tagged_filters_all: &blocker.tagged_filters_all,

            enable_optimizations: blocker.enable_optimizations,

            resources: &blocker.resources,

            simple_class_rules: &cfc.simple_class_rules,
            simple_id_rules: &cfc.simple_id_rules,
            complex_class_rules: &cfc.complex_class_rules,
            complex_id_rules: &cfc.complex_id_rules,

            specific_rules: &cfc.specific_rules,

            misc_generic_selectors: &cfc.misc_generic_selectors,

            scriptlets: &cfc.scriptlets,
        }
    }
}

impl From<DeserializeFormat> for (Blocker, CosmeticFilterCache) {
    fn from(v: DeserializeFormat) -> Self {
        (Blocker {
            csp: v.csp.into(),
            exceptions: v.exceptions.into(),
            importants: v.importants.into(),
            redirects: v.redirects.into(),
            removeparam: NetworkFilterList::default(),
            filters_tagged: v.filters_tagged.into(),
            filters: v.filters.into(),
            generic_hide: v.generic_hide.into(),

            tags_enabled: Default::default(),
            tagged_filters_all: v.tagged_filters_all.into_iter().map(|f| f.into()).collect(),

            enable_optimizations: v.enable_optimizations,

            resources: v.resources,
            #[cfg(feature = "object-pooling")]
            pool: Default::default(),

        }, CosmeticFilterCache {
            simple_class_rules: v.simple_class_rules,
            simple_id_rules: v.simple_id_rules,
            complex_class_rules: v.complex_class_rules,
            complex_id_rules: v.complex_id_rules,

            specific_rules: v.specific_rules,

            misc_generic_selectors: v.misc_generic_selectors,

            scriptlets: v.scriptlets,
        })
    }
}
