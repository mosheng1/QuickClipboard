// 剪贴板内容类型管理模块

// 内容类型
#[derive(Debug, Clone)]
pub struct ContentType {
    types: Vec<String>,
}

impl ContentType {
    pub fn new(primary_type: &str) -> Self {
        Self {
            types: vec![primary_type.to_string()],
        }
    }

    pub fn add_type(&mut self, type_name: &str) {
        if !self.types.contains(&type_name.to_string()) {
            self.types.push(type_name.to_string());
        }
    }

    pub fn to_db_string(&self) -> String {
        self.types.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ct = ContentType::new("text");
        assert_eq!(ct.to_db_string(), "text");
    }

    #[test]
    fn test_add_type() {
        let mut ct = ContentType::new("rich_text");
        ct.add_type("link");
        assert_eq!(ct.to_db_string(), "rich_text,link");
    }
}
