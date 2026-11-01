use sqlx::{Database, QueryBuilder};

#[derive(Debug, Clone)]
enum AttributeValue {
    Int(i64),
    Text(String),
}

#[derive(Debug, Clone, Copy)]
enum Operator {
    Eq,
    Gt,
    Like,
}

impl Operator {
    pub(crate) fn as_sql(self) -> &'static str {
        match self {
            Operator::Eq => "=",
            Operator::Gt => ">",
            Operator::Like => "LIKE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Predicate {
    column: &'static str,
    operator: Operator,
    value: AttributeValue,
}

pub struct IntColumn {
    name: &'static str,
}

impl IntColumn {
    pub fn eq(&self, v: i64) -> Predicate {
        Predicate {
            column: self.name,
            operator: Operator::Eq,
            value: AttributeValue::Int(v),
        }
    }

    pub fn gt(&self, v: i64) -> Predicate {
        Predicate {
            column: self.name,
            operator: Operator::Gt,
            value: AttributeValue::Int(v),
        }
    }
}

pub struct StringColumn {
    name: &'static str,
}

impl StringColumn {
    pub fn like(&self, v: &str) -> Predicate {
        Predicate {
            column: self.name,
            operator: Operator::Like,
            value: AttributeValue::Text(String::from(v)),
        }
    }
}

pub struct Columns {
    pub process: IntColumn,
    pub project_id: IntColumn,
    pub channel: IntColumn,
    pub datamode: IntColumn,
    pub datatype: IntColumn,
    pub filepath: StringColumn,
}

pub const COLUMNS: Columns = Columns {
    process: IntColumn { name: "process" },
    project_id: IntColumn { name: "project_id" },
    channel: IntColumn { name: "channel" },
    datamode: IntColumn { name: "datamode" },
    datatype: IntColumn { name: "datatype" },
    filepath: StringColumn { name: "filepath" },
};

#[derive(Default)]
pub struct MetadataFilter {
    predicates: Vec<Predicate>,
}

impl MetadataFilter {
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
        }
    }

    pub fn and(mut self, p: Predicate) -> Self {
        self.predicates.push(p);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn get_predicates(&self) -> &[Predicate] {
        &self.predicates
    }

    pub(crate) fn apply<'a, DB>(&'a self, builder: &mut QueryBuilder<DB>)
    where
        DB: Database,
        i64: sqlx::Type<DB> + sqlx::Encode<'a, DB>,
        String: sqlx::Type<DB> + sqlx::Encode<'a, DB>,
    {
        if self.predicates.is_empty() {
            return;
        }
        builder.push(" WHERE ");
        let mut sep = builder.separated(" AND ");
        for predicate in &self.predicates {
            sep.push(predicate.column)
                .push_unseparated(" ")
                .push_unseparated(predicate.operator.as_sql())
                .push_unseparated(" ");
            match &predicate.value {
                AttributeValue::Int(n) => sep.push_bind_unseparated(n),
                AttributeValue::Text(s) => sep.push_bind_unseparated(s),
            };
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sqlx::Sqlite;

    #[test]
    fn int_eq_predicate() {
        let p = COLUMNS.process.eq(1);
        assert_eq!(p.column, "process");
        assert!(matches!(p.operator, Operator::Eq));
        assert!(matches!(p.value, AttributeValue::Int(1)));
    }

    #[test]
    fn string_like_predicate() {
        let p = COLUMNS.filepath.like("foo");
        assert_eq!(p.column, "filepath");
        assert!(matches!(p.operator, Operator::Like));
        assert!(matches!(p.value, AttributeValue::Text(ref s) if s == "foo"));
    }

    #[test]
    fn test_apply_query_builder() {
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM metadata");
        let filter = MetadataFilter::new().and(COLUMNS.process.eq(1));
        filter.apply(&mut builder);
        let query = builder.into_string();
        assert_eq!(
            String::from("SELECT * FROM metadata WHERE process = ?"),
            query
        );
    }

    #[test]
    fn test_apply_query_builder_multiple_predicates() {
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM metadata");
        let filter = MetadataFilter::new()
            .and(COLUMNS.process.eq(1))
            .and(COLUMNS.filepath.like("foo"));
        filter.apply(&mut builder);
        let query = builder.into_string();
        assert_eq!(
            "SELECT * FROM metadata WHERE process = ? AND filepath LIKE ?",
            query
        );
    }
}
