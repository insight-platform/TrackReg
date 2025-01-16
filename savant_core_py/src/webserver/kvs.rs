use crate::primitives::attribute::Attribute;
use crate::{release_gil, with_gil};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use savant_core::primitives::rust;
use savant_core::primitives::rust::AttributeSet;
use savant_core::protobuf::ToProtobuf;
use savant_core::webserver::kvs::synchronous as sync_kvs;

/// Set attributes in the key-value store.
///
/// Parameters
/// ----------
/// attributes : List[Attribute]
///  List of attributes to set.
///
/// ttl : Optional[int]
///  Time-to-live for the attributes.
///
#[pyfunction]
#[pyo3(signature = (attributes, ttl=None))]
pub fn set_attributes(attributes: Vec<Attribute>, ttl: Option<u64>) {
    let attributes =
        unsafe { std::mem::transmute::<Vec<Attribute>, Vec<rust::Attribute>>(attributes) };
    sync_kvs::set_attributes(&attributes, ttl);
}

/// Search for attributes in the key-value store.
///
/// Parameters
/// ----------
/// ns : Optional[str]
///  Namespace to search for (Glob). None means "*".
///
/// name : Optional[str]
///  Name to search for (Glob). None means "*".
///
/// Returns
/// -------
/// List[Attribute]
///   List of attributes found.
///
#[pyfunction]
#[pyo3(signature = (ns=None, name=None, no_gil=false))]
pub fn search_attributes(ns: Option<String>, name: Option<String>, no_gil: bool) -> Vec<Attribute> {
    release_gil!(no_gil, || {
        let attributes = sync_kvs::search_attributes(&ns, &name);
        unsafe { std::mem::transmute::<Vec<rust::Attribute>, Vec<Attribute>>(attributes) }
    })
}

/// Search for keys in the key-value store.
///
/// Parameters
/// ----------
/// ns : Optional[str]
///  Namespace to search for (Glob). None means "*".
///
/// name : Optional[str]
///  Name to search for (Glob). None means "*".
///
/// Returns
/// -------
/// List[Tuple[str, str]]
///  List of keys found.
///
#[pyfunction]
#[pyo3(signature = (ns=None, name=None, no_gil=false))]
pub fn search_keys(
    ns: Option<String>,
    name: Option<String>,
    no_gil: bool,
) -> Vec<(String, String)> {
    release_gil!(no_gil, || { sync_kvs::search_keys(&ns, &name) })
}

/// Delete attributes from the key-value store.
///
/// Parameters
/// ----------
/// ns : Optional[str]
///  Namespace to delete from (Glob). None means "*".
///
/// name : Optional[str]
///  Name to delete (Glob). None means "*".
///
#[pyfunction]
#[pyo3(signature = (ns=None, name=None, no_gil=false))]
pub fn del_attributes(ns: Option<String>, name: Option<String>, no_gil: bool) {
    release_gil!(no_gil, || {
        sync_kvs::del_attributes(&ns, &name);
    });
}

/// Get an attribute from the key-value store.
///
/// Parameters
/// ----------
/// ns : str
///  Namespace to get from.
///
/// name : str
///  Name to get.
///
/// Returns
/// -------
/// Optional[Attribute]
///  The attribute found.
///
#[pyfunction]
pub fn get_attribute(ns: &str, name: &str) -> Option<Attribute> {
    sync_kvs::get_attribute(ns, name).map(Attribute)
}

/// Delete an attribute from the key-value store.
///
/// Parameters
/// ----------
/// ns : str
///  Namespace to delete from.
///
/// name : str
///  Name to delete.
///
/// Returns
/// -------
/// Optional[Attribute]
///  The attribute deleted.
///
#[pyfunction]
pub fn del_attribute(ns: &str, name: &str) -> Option<Attribute> {
    sync_kvs::del_attribute(ns, name).map(Attribute)
}

/// Serialize a list of attributes to a byte buffer.
///
/// Parameters
/// ----------
/// attributes : List[Attribute]
///  List of attributes to serialize.
///
/// Returns
/// -------
/// bytes
///  The serialized attributes.
///
/// Raises
/// ------
/// ValueError
///  If serialization fails.
///
#[pyfunction]
pub fn serialize_attributes(attributes: Vec<Attribute>) -> PyResult<PyObject> {
    let attributes =
        unsafe { std::mem::transmute::<Vec<Attribute>, Vec<rust::Attribute>>(attributes) };
    let attr_set = AttributeSet::from(attributes);
    let res = attr_set
        .to_pb()
        .map_err(|e| PyValueError::new_err(e.to_string()))?;

    with_gil!(|py| {
        let bytes = PyBytes::new_with(py, res.len(), |b: &mut [u8]| {
            b.copy_from_slice(res.as_slice());
            Ok(())
        })?;
        Ok(PyObject::from(bytes))
    })
}

/// Deserialize a byte buffer to a list of attributes.
///
/// Parameters
/// ----------
/// serialized : bytes
///  The serialized attributes.
///
/// Returns
/// -------
/// List[Attribute]
///  The deserialized attributes.
///
/// Raises
/// ------
/// ValueError
///  If deserialization fails.
///
#[pyfunction]
pub fn deserialize_attributes(serialized: &Bound<'_, PyBytes>) -> PyResult<Vec<Attribute>> {
    let bytes = serialized.as_bytes();
    let attributes =
        AttributeSet::deserialize(bytes).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(unsafe { std::mem::transmute::<Vec<rust::Attribute>, Vec<Attribute>>(attributes) })
}