pub mod asynchronous {
    use crate::primitives::attribute::Attribute;
    use crate::webserver::WS_DATA;
    use globset::Glob;

    pub async fn set_attributes(attributes: &[Attribute], ttl: Option<u64>) {
        for attr in attributes {
            let ns = attr.namespace.clone();
            let name = attr.name.clone();
            WS_DATA
                .kvs
                .get_with((ns, name), async { (ttl, attr.clone()) })
                .await;
        }
    }

    pub async fn search_attributes(ns: &Option<String>, name: &Option<String>) -> Vec<Attribute> {
        let ns_glob = ns
            .as_ref()
            .map(|s| Glob::new(s.as_str()))
            .unwrap_or(Glob::new("*"))
            .unwrap()
            .compile_matcher();

        let name_glob = name
            .as_ref()
            .map(|s| Glob::new(s.as_str()))
            .unwrap_or(Glob::new("*"))
            .unwrap()
            .compile_matcher();

        let mut attr_set = Vec::new();
        for (key, (_, attr)) in WS_DATA.kvs.iter() {
            let key_ns = &key.0;
            let key_name = &key.1;
            if ns_glob.is_match(key_ns) && name_glob.is_match(key_name) {
                attr_set.push(attr.clone());
            }
        }
        attr_set
    }

    pub async fn search_keys(ns: &Option<String>, name: &Option<String>) -> Vec<(String, String)> {
        let mut keys = Vec::new();
        let ns_glob = ns
            .as_ref()
            .map(|s| Glob::new(s.as_str()))
            .unwrap_or(Glob::new("*"))
            .unwrap()
            .compile_matcher();

        let name_glob = name
            .as_ref()
            .map(|s| Glob::new(s.as_str()))
            .unwrap_or(Glob::new("*"))
            .unwrap()
            .compile_matcher();

        for (key, _) in WS_DATA.kvs.iter() {
            let key_ns = &key.0;
            let key_name = &key.1;
            if ns_glob.is_match(key_ns) && name_glob.is_match(key_name) {
                keys.push((key_ns.clone(), key_name.clone()));
            }
        }
        keys
    }

    pub async fn del_attributes(ns: &Option<String>, name: &Option<String>) {
        let mut keys_to_delete = Vec::new();
        let ns_glob = ns
            .as_ref()
            .map(|s| Glob::new(s.as_str()))
            .unwrap_or(Glob::new("*"))
            .unwrap()
            .compile_matcher();

        let name_glob = name
            .as_ref()
            .map(|s| Glob::new(s.as_str()))
            .unwrap_or(Glob::new("*"))
            .unwrap()
            .compile_matcher();

        for (key, _) in WS_DATA.kvs.iter() {
            let key_ns = &key.0;
            let key_name = &key.1;
            if ns_glob.is_match(key_ns) && name_glob.is_match(key_name) {
                keys_to_delete.push(key.clone());
            }
        }
        for key in keys_to_delete {
            WS_DATA.kvs.remove(&key).await;
        }
    }

    pub async fn get_attribute(ns: &str, name: &str) -> Option<Attribute> {
        WS_DATA
            .kvs
            .get(&(ns.to_string(), name.to_string()))
            .await
            .map(|(_, attr)| attr)
    }

    pub async fn del_attribute(ns: &str, name: &str) -> Option<Attribute> {
        WS_DATA
            .kvs
            .remove(&(ns.to_string(), name.to_string()))
            .await
            .map(|(_, attr)| attr)
    }
}

pub mod synchronous {
    use crate::get_or_init_async_runtime;
    use crate::primitives::attribute::Attribute;

    pub fn set_attributes(attributes: &[Attribute], ttl: Option<u64>) {
        let rt = get_or_init_async_runtime();
        rt.block_on(async {
            crate::webserver::kvs::asynchronous::set_attributes(attributes, ttl).await
        });
    }

    pub fn search_attributes(ns: &Option<String>, name: &Option<String>) -> Vec<Attribute> {
        let rt = get_or_init_async_runtime();
        rt.block_on(async {
            crate::webserver::kvs::asynchronous::search_attributes(ns, name).await
        })
    }

    pub fn search_keys(ns: &Option<String>, name: &Option<String>) -> Vec<(String, String)> {
        let rt = get_or_init_async_runtime();
        rt.block_on(async { crate::webserver::kvs::asynchronous::search_keys(ns, name).await })
    }

    pub fn del_attributes(ns: &Option<String>, name: &Option<String>) {
        let rt = get_or_init_async_runtime();
        rt.block_on(async { crate::webserver::kvs::asynchronous::del_attributes(ns, name).await });
    }

    pub fn get_attribute(ns: &str, name: &str) -> Option<Attribute> {
        let rt = get_or_init_async_runtime();
        rt.block_on(async { crate::webserver::kvs::asynchronous::get_attribute(ns, name).await })
    }

    pub fn del_attribute(ns: &str, name: &str) -> Option<Attribute> {
        let rt = get_or_init_async_runtime();
        rt.block_on(async { crate::webserver::kvs::asynchronous::del_attribute(ns, name).await })
    }
}

#[cfg(test)]
mod tests {
    use crate::primitives::attribute::Attribute;
    use crate::webserver::kvs::synchronous::*;
    use std::thread::sleep;

    #[test]
    fn test_kvs() {
        let attribute_set = vec![
            Attribute::persistent("abc", "xax", vec![], &None, false),
            Attribute::persistent("ghi", "yay", vec![], &None, false),
        ];
        set_attributes(&attribute_set, None);
        let retrieved_all = search_attributes(&None, &None);
        assert_eq!(retrieved_all.len(), 2);
        let retrieved_abc = search_attributes(&Some("abc".to_string()), &None);
        assert_eq!(retrieved_abc.len(), 1);
        let retrieved_with_glob = search_attributes(&None, &Some("?a?".to_string()));
        assert_eq!(retrieved_with_glob.len(), 2);

        let ttl_attribute_set = vec![
            Attribute::persistent("def", "xax", vec![], &None, false),
            Attribute::persistent("jkl", "yay", vec![], &None, false),
        ];

        set_attributes(&ttl_attribute_set, Some(10));
        let retrieved_all = search_attributes(&None, &None);
        assert_eq!(retrieved_all.len(), 4);
        sleep(std::time::Duration::from_millis(11));
        let retrieved_all = search_attributes(&None, &None);
        assert_eq!(retrieved_all.len(), 2);

        let abc_attribute = get_attribute(&"abc".to_string(), &"xax".to_string());
        assert_eq!(abc_attribute.as_ref().unwrap().name.as_str(), "xax");

        del_attributes(&None, &None);
        let retrieved_all = search_attributes(&None, &None);
        assert_eq!(retrieved_all.len(), 0);
    }
}
