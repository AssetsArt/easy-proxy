use super::get_database;
use surrealdb::Response;

enum Engine {
    Disk,
    Memory,
}

#[derive(Debug, Clone)]
pub struct SqlBuilder {
    pub query: Vec<String>,
    pub binds: Vec<(String, String)>,
    pub table: String,
    pub statement: Vec<String>,
}

impl Default for SqlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlBuilder {
    pub fn new() -> Self {
        SqlBuilder {
            query: Vec::new(),
            binds: Vec::new(),
            table: String::new(),
            statement: Vec::new(),
        }
    }

    pub fn table(&mut self, table: &str) -> Self {
        self.table = table.to_string();
        self.clone()
    }

    pub fn select(&mut self, field: Vec<String>) -> Self {
        let mut fields = String::new();
        for f in field {
            // fist
            if fields.is_empty() {
                fields = f.to_string();
                continue;
            }
            fields = format!("{}, {}", fields, f);
        }
        fields = fields.trim().to_string();
        self.query
            .push(format!("SELECT {} FROM {}", fields, self.table));
        self.clone()
    }

    pub fn r#where(&mut self, field: &str, value: &str) -> Self {
        if self.query.is_empty() {
            self.query.push(format!("SELECT * FROM {}", self.table));
        }
        if self.statement.is_empty() {
            self.statement.push(format!("WHERE {} = ${}", field, field));
        } else {
            self.statement.push(format!("AND {} = ${}", field, field));
        }
        self.binds.push((field.to_string(), value.to_string()));
        self.clone()
    }

    pub fn crypto_compare(&mut self, field: &str, value: &str) -> Self {
        // crypto::argon2::compare(password, $password)
        if self.query.is_empty() {
            self.query.push(format!("SELECT * FROM {}", self.table));
        }
        if self.statement.is_empty() {
            self.statement.push(format!(
                "WHERE crypto::argon2::compare({}, ${})",
                field, field
            ));
        } else {
            self.statement.push(format!(
                "AND crypto::argon2::compare({}, ${})",
                field, field
            ));
        }
        self.binds.push((field.to_string(), value.to_string()));
        self.clone()
    }

    async fn builder(
        &self,
        state: Engine,
    ) -> surrealdb::method::Query<'_, surrealdb::engine::local::Db> {
        let dbs = get_database().await;
        let db = match state {
            Engine::Disk => &dbs.disk,
            Engine::Memory => &dbs.memory,
        };
        let mut query = String::new();
        for q in &self.query {
            query = format!("{} {}", query, q);
        }
        let mut statement = String::new();
        for s in &self.statement {
            statement = format!("{} {}", statement, s);
        }
        let query = format!("{} {}", query, statement);
        // println!("query: {}", query);
        let mut result = db.query(&query);
        for b in &self.binds {
            result = result.bind((b.0.as_str(), b.1.as_str()));
        }
        result
    }

    pub async fn disk_execute(&self) -> Result<Response, surrealdb::Error> {
        let result = self.builder(Engine::Disk).await;
        if self.table == String::new() {
            return Err(surrealdb::Error::Db(surrealdb::error::Db::InvalidParam {
                name: "Table is not set".to_string(),
            }));
        }
        match result.await {
            Ok(r) => Ok(r),
            Err(e) => Err(e),
        }
    }

    pub async fn mem_execute(&self) -> Result<Response, surrealdb::Error> {
        let result = self.builder(Engine::Memory).await;
        if self.table == String::new() {
            return Err(surrealdb::Error::Db(surrealdb::error::Db::InvalidParam {
                name: "Table is not set".to_string(),
            }));
        }
        match result.await {
            Ok(r) => Ok(r),
            Err(e) => Err(e),
        }
    }
}
