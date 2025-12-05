use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CouchMeta {
  #[serde(alias = "_id")]
  pub id: Option<String>,
  #[serde(alias = "_rev")]
  pub rev: Option<String>,
  pub document_type: Option<String>,
}

#[allow(dead_code)]
pub trait CouchModelExt: Serialize {
  fn document_type(&self) -> Option<&'static str> {
    None
  }

  fn document_type_str(&self) -> Option<&str> {
    self.document_type()
  }

  /// JSON для CouchDB: _id / _rev, без null, с document_type
  fn dump_for_db(&self) -> Value {
    let mut v = serde_json::to_value(self).expect("serialize to Value failed");

    if let Some(doc_type) = self.document_type_str()
      && let Value::Object(ref mut map) = v
    {
      map.insert(
        "document_type".to_string(),
        Value::String(doc_type.to_string()),
      );
    }

    if let Value::Object(ref mut map) = v {
      if let Some(id_val) = map.remove("id")
        && !id_val.is_null()
      {
        map.insert("_id".to_string(), id_val);
      }

      if let Some(rev_val) = map.remove("rev")
        && !rev_val.is_null()
      {
        map.insert("_rev".to_string(), rev_val);
      }
    }

    strip_nulls(&mut v);
    v
  }

  /// Публичный JSON: id, без rev/document_type, без пустых id/_id
  fn to_public_json(&self) -> Value {
    let mut v = serde_json::to_value(self).expect("serialize to Value failed");

    if let Value::Object(ref mut map) = v {
      map.remove("rev");
      map.remove("document_type");

      // если вдруг прилетело _id (из базы), перегоняем в id
      if let Some(id_val) = map.remove("_id")
        && !id_val.is_null()
      {
        map.insert("id".to_string(), id_val);
      }
    }

    strip_empty_ids(&mut v);
    v
  }
}

fn strip_empty_ids(value: &mut Value) {
  match value {
    Value::Object(map) => {
      let keys: Vec<String> = map.keys().cloned().collect();
      for k in keys {
        if let Some(v) = map.get_mut(&k) {
          if (k == "id" || k == "_id") && v.is_null() {
            map.remove(&k);
          } else {
            strip_empty_ids(v);
          }
        }
      }
    }
    Value::Array(arr) => {
      for v in arr {
        strip_empty_ids(v);
      }
    }
    _ => {}
  }
}

fn strip_nulls(value: &mut Value) {
  match value {
    Value::Object(map) => {
      let keys: Vec<String> = map.keys().cloned().collect();
      for k in keys {
        let remove_this;
        {
          let v = map.get_mut(&k).unwrap();
          if v.is_null() {
            remove_this = true;
          } else {
            remove_this = false;
            strip_nulls(v);
          }
        }
        if remove_this {
          map.remove(&k);
        }
      }
    }
    Value::Array(arr) => {
      arr.retain(|v| !v.is_null());
      for v in arr {
        strip_nulls(v);
      }
    }
    _ => {}
  }
}
