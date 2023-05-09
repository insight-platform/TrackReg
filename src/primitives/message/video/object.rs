use crate::primitives::{Attribute, BBox};
use pyo3::{pyclass, pymethods, Py, PyAny, Python};
use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[pyclass]
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone)]
#[archive(check_bytes)]
pub struct ParentObject {
    #[pyo3(get, set)]
    pub id: i64,
    #[pyo3(get, set)]
    pub creator: String,
    #[pyo3(get, set)]
    pub label: String,
}

#[pymethods]
impl ParentObject {
    #[classattr]
    const __hash__: Option<Py<PyAny>> = None;

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    #[new]
    pub fn new(id: i64, creator: String, label: String) -> Self {
        Self { id, creator, label }
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone, derive_builder::Builder)]
#[archive(check_bytes)]
pub(crate) struct InnerObject {
    pub id: i64,
    pub creator: String,
    pub label: String,
    pub bbox: BBox,
    pub attributes: HashMap<(String, String), Attribute>,
    pub confidence: Option<f64>,
    pub parent: Option<ParentObject>,
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct Object {
    pub(crate) inner: Arc<Mutex<InnerObject>>,
}

impl Object {
    #[cfg(test)]
    pub(crate) fn from_object(object: InnerObject) -> Self {
        Self {
            inner: Arc::new(Mutex::new(object)),
        }
    }

    pub(crate) fn from_arc_object(object: Arc<Mutex<InnerObject>>) -> Self {
        Self { inner: object }
    }
}

#[pymethods]
impl Object {
    #[classattr]
    const __hash__: Option<Py<PyAny>> = None;

    fn __repr__(&self) -> String {
        format!("{:#?}", self.inner.lock().unwrap())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    #[new]
    pub fn new(
        id: i64,
        creator: String,
        label: String,
        bbox: BBox,
        attributes: HashMap<(String, String), Attribute>,
        confidence: Option<f64>,
        parent: Option<ParentObject>,
    ) -> Self {
        let object = InnerObject {
            id,
            creator,
            label,
            bbox,
            attributes,
            confidence,
            parent,
        };
        Self {
            inner: Arc::new(Mutex::new(object)),
        }
    }

    pub fn id(&self) -> i64 {
        self.inner.lock().unwrap().id
    }

    pub fn creator(&self) -> String {
        self.inner.lock().unwrap().creator.clone()
    }

    pub fn label(&self) -> String {
        self.inner.lock().unwrap().label.clone()
    }

    pub fn bbox(&self) -> crate::primitives::BBox {
        self.inner.lock().unwrap().bbox.clone()
    }

    pub fn confidence(&self) -> Option<f64> {
        let object = self.inner.lock().unwrap();
        object.confidence
    }

    pub fn parent(&self) -> Option<ParentObject> {
        let object = self.inner.lock().unwrap();
        object.parent.clone()
    }

    pub fn set_id(&mut self, id: i64) {
        let mut object = self.inner.lock().unwrap();
        object.id = id;
    }

    pub fn set_creator(&mut self, creator: String) {
        let mut object = self.inner.lock().unwrap();
        object.creator = creator;
    }

    pub fn set_label(&mut self, label: String) {
        let mut object = self.inner.lock().unwrap();
        object.label = label;
    }

    pub fn set_bbox(&mut self, bbox: BBox) {
        self.inner.lock().unwrap().bbox = bbox;
    }

    pub fn set_confidence(&mut self, confidence: Option<f64>) {
        let mut object = self.inner.lock().unwrap();
        object.confidence = confidence;
    }

    pub fn set_parent(&mut self, parent: Option<ParentObject>) {
        let mut object = self.inner.lock().unwrap();
        object.parent = parent;
    }

    pub fn attributes(&self) -> Vec<(String, String)> {
        Python::with_gil(|py| {
            py.allow_threads(|| {
                let object = self.inner.lock().unwrap();
                object
                    .attributes
                    .iter()
                    .map(|((creator, name), _)| (creator.clone(), name.clone()))
                    .collect()
            })
        })
    }

    pub fn get_attribute(&self, creator: String, name: String) -> Option<Attribute> {
        let object = self.inner.lock().unwrap();
        object.attributes.get(&(creator, name)).cloned()
    }

    pub fn delete_attribute(&mut self, creator: String, name: String) -> Option<Attribute> {
        let mut object = self.inner.lock().unwrap();
        object.attributes.remove(&(creator, name))
    }

    pub fn set_attribute(&mut self, attribute: Attribute) -> Option<Attribute> {
        let mut object = self.inner.lock().unwrap();
        object.attributes.insert(
            (attribute.creator.clone(), attribute.name.clone()),
            attribute,
        )
    }

    pub fn clear_attributes(&mut self) {
        let mut object = self.inner.lock().unwrap();
        object.attributes.clear();
    }

    #[pyo3(signature = (negated=false, creator=None, names=vec![]))]
    pub fn delete_attributes(
        &mut self,
        negated: bool,
        creator: Option<String>,
        names: Vec<String>,
    ) {
        Python::with_gil(|py| {
            py.allow_threads(|| {
                let mut object = self.inner.lock().unwrap();
                object.attributes.retain(|(c, label), _| match creator {
                    Some(ref creator) => {
                        ((names.is_empty() || names.contains(label)) && creator == c) ^ !negated
                    }
                    None => names.contains(label) ^ !negated,
                });
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::primitives::message::video::object::InnerObjectBuilder;
    use crate::primitives::{AttributeBuilder, BBox, Object, Value};

    fn get_object() -> Object {
        Object::from_object(
            InnerObjectBuilder::default()
                .id(1)
                .creator("model".to_string())
                .label("label".to_string())
                .bbox(BBox::new(0.0, 0.0, 1.0, 1.0, None))
                .confidence(Some(0.5))
                .attributes(
                    vec![
                        AttributeBuilder::default()
                            .creator("creator".to_string())
                            .name("name".to_string())
                            .value(Value::string("value".to_string()))
                            .confidence(None)
                            .hint(None)
                            .build()
                            .unwrap(),
                        AttributeBuilder::default()
                            .creator("creator".to_string())
                            .name("name2".to_string())
                            .value(Value::string("value2".to_string()))
                            .confidence(None)
                            .hint(None)
                            .build()
                            .unwrap(),
                        AttributeBuilder::default()
                            .creator("creator2".to_string())
                            .name("name".to_string())
                            .value(Value::string("value".to_string()))
                            .confidence(None)
                            .hint(None)
                            .build()
                            .unwrap(),
                    ]
                    .into_iter()
                    .map(|a| ((a.creator.clone(), a.name.clone()), a))
                    .collect(),
                )
                .parent(None)
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn test_delete_attributes() {
        pyo3::prepare_freethreaded_python();

        let mut t = get_object();
        t.delete_attributes(false, None, vec![]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 3);

        let mut t = get_object();
        t.delete_attributes(true, None, vec![]);
        assert!(t.inner.lock().unwrap().attributes.is_empty());

        let mut t = get_object();
        t.delete_attributes(false, Some("creator".to_string()), vec![]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 1);

        let mut t = get_object();
        t.delete_attributes(true, Some("creator".to_string()), vec![]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 2);

        let mut t = get_object();
        t.delete_attributes(false, None, vec!["name".to_string()]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 1);

        let mut t = get_object();
        t.delete_attributes(true, None, vec!["name".to_string()]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 2);

        let mut t = get_object();
        t.delete_attributes(false, None, vec!["name".to_string(), "name2".to_string()]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 0);

        let mut t = get_object();
        t.delete_attributes(true, None, vec!["name".to_string(), "name2".to_string()]);
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 3);

        let mut t = get_object();
        t.delete_attributes(
            false,
            Some("creator".to_string()),
            vec!["name".to_string(), "name2".to_string()],
        );
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 1);

        assert_eq!(
            &t.inner.lock().unwrap().attributes[&("creator2".to_string(), "name".to_string())],
            &AttributeBuilder::default()
                .creator("creator2".to_string())
                .name("name".to_string())
                .value(Value::string("value".to_string()))
                .confidence(None)
                .hint(None)
                .build()
                .unwrap()
        );

        let mut t = get_object();
        t.delete_attributes(
            true,
            Some("creator".to_string()),
            vec!["name".to_string(), "name2".to_string()],
        );
        assert_eq!(t.inner.lock().unwrap().attributes.len(), 2);
    }
}
