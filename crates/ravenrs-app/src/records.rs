//! Records many applications need, in their own unit of work: numbers that follow each other and
//! work kept to pick up later.

use ravenrs_criteria::Value;

use crate::{EntityInfo, FieldInfo, FieldKind, Module, Object, ObjectSpace, Query, SpaceError};

/// The entity that holds the last number of each sequence.
pub const SEQUENCE_ENTITY: &str = "AppSequence";
/// The entity that holds work kept to pick up later.
pub const DRAFT_ENTITY: &str = "AppDraft";

/// The entity of sequences, which storage has to know.
pub fn sequence_entity() -> EntityInfo {
    EntityInfo::new(SEQUENCE_ENTITY)
        .caption("Sequences")
        .field(FieldInfo::new("name", FieldKind::Text).typed_key())
        .field(FieldInfo::new("last", FieldKind::Integer).default_value(0.0))
}

/// The entity of drafts, which storage has to know.
pub fn draft_entity() -> EntityInfo {
    EntityInfo::new(DRAFT_ENTITY)
        .caption("Drafts")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("kind", FieldKind::Text))
        .field(FieldInfo::new("title", FieldKind::Text).default_property())
        .field(FieldInfo::new("payload", FieldKind::LongText))
}

/// The next number of a sequence, kept with the rest of the unit of work: when that is thrown away
/// the number comes again, so a commit leaves no gaps.
pub fn next_number(space: &mut dyn ObjectSpace, sequence: &str) -> Result<i64, SpaceError> {
    let key = Value::Text(sequence.to_owned());
    let mut record = match space.find(SEQUENCE_ENTITY, &key)? {
        Some(record) => record,
        None => {
            let mut record = space.new_object(SEQUENCE_ENTITY)?;
            record.set("name", sequence.to_owned());
            record
        }
    };
    let next = match record.get("last") {
        Value::Number(last) => last as i64 + 1,
        _ => 1,
    };
    record.set("last", next as f64);
    space.save(record)?;
    Ok(next)
}

/// Work kept to pick up later.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Draft {
    /// What it is known by.
    pub id: Value,
    /// What sort of work it is, for whoever picks it up.
    pub kind: String,
    /// How a person tells it apart.
    pub title: String,
    /// The work itself, in whatever text its owner reads back.
    pub payload: String,
}

fn draft_of(record: &Object) -> Draft {
    let text = |name: &str| match record.get(name) {
        Value::Text(text) => text,
        _ => String::new(),
    };
    Draft { id: record.get("id"), kind: text("kind"), title: text("title"), payload: text("payload") }
}

/// Keeps this work, in the unit of work.
pub fn save_draft(
    space: &mut dyn ObjectSpace,
    kind: &str,
    title: &str,
    payload: &str,
) -> Result<Value, SpaceError> {
    let mut record = space.new_object(DRAFT_ENTITY)?;
    record.set("kind", kind.to_owned());
    record.set("title", title.to_owned());
    record.set("payload", payload.to_owned());
    let id = record.get("id");
    space.save(record)?;
    Ok(id)
}

/// The work kept of this kind.
pub fn drafts(space: &dyn ObjectSpace, kind: &str) -> Result<Vec<Draft>, SpaceError> {
    Ok(space
        .query(DRAFT_ENTITY, &Query::all())?
        .iter()
        .map(draft_of)
        .filter(|draft| draft.kind == kind)
        .collect())
}

/// Hands back this work and removes it, in the unit of work.
pub fn take_draft(space: &mut dyn ObjectSpace, id: &Value) -> Result<Option<Draft>, SpaceError> {
    let Some(record) = space.find(DRAFT_ENTITY, id)? else { return Ok(None) };
    space.delete(DRAFT_ENTITY, id)?;
    Ok(Some(draft_of(&record)))
}

/// Brings sequences and drafts to an application.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AppRecordsModule;

impl Module for AppRecordsModule {
    fn name(&self) -> &str {
        "AppRecords"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![sequence_entity(), draft_entity()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryDatabase;

    fn database() -> MemoryDatabase {
        MemoryDatabase::new(vec![sequence_entity(), draft_entity()]).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn numbers_follow_each_other_and_one_thrown_away_comes_again() {
        let database = database();
        let mut space = database.space();
        assert_eq!(next_number(&mut space, "venda").ok(), Some(1));
        assert_eq!(next_number(&mut space, "venda").ok(), Some(2));
        assert_eq!(next_number(&mut space, "outra").ok(), Some(1));
        space.commit().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(next_number(&mut space, "venda").ok(), Some(3));
        space.rollback();
        assert_eq!(next_number(&mut space, "venda").ok(), Some(3));
    }

    #[test]
    fn a_draft_comes_back_once() {
        let database = database();
        let mut space = database.space();
        let id = save_draft(&mut space, "venda", "Venda 12", "{}").unwrap_or_else(|e| panic!("{e}"));
        save_draft(&mut space, "outro", "Outro", "x").unwrap_or_else(|e| panic!("{e}"));
        space.commit().unwrap_or_else(|e| panic!("{e}"));
        let kept = drafts(&space, "venda").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].title, "Venda 12");
        let taken = take_draft(&mut space, &id).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(taken.map(|draft| draft.payload).as_deref(), Some("{}"));
        space.commit().unwrap_or_else(|e| panic!("{e}"));
        assert!(drafts(&space, "venda").unwrap_or_else(|e| panic!("{e}")).is_empty());
    }
}
