use crate::pk::PrimaryKey;
use crate::*;
use itertools::Itertools;
use once_cell::sync::OnceCell;
use std::{
    collections::BTreeSet,
    sync::{Arc, Weak},
};

#[derive(Debug, Clone)]
pub struct Fields {
    pub all: Vec<Field>,
    primary_key: Option<PrimaryKey>,
    scalar: OnceCell<Vec<ScalarFieldWeak>>,
    relation: OnceCell<Vec<RelationFieldWeak>>,
    composite: OnceCell<Vec<CompositeFieldWeak>>,
    model: ModelWeakRef,
    // created_at: OnceCell<Option<ScalarFieldRef>>,
    updated_at: OnceCell<Option<ScalarFieldRef>>,
}

impl Fields {
    pub fn new(all: Vec<Field>, model: ModelWeakRef, primary_key: Option<PrimaryKey>) -> Fields {
        Fields {
            all,
            primary_key,
            scalar: OnceCell::new(),
            relation: OnceCell::new(),
            composite: OnceCell::new(),
            // created_at: OnceCell::new(),
            updated_at: OnceCell::new(),
            model,
        }
    }

    pub(crate) fn finalize(&self) {
        self.mark_read_only()
    }

    fn mark_read_only(&self) {
        let inlined_rfs: Vec<&RelationFieldRef> = self
            .all
            .iter()
            .filter_map(|f| match f {
                Field::Relation(rf) if rf.is_inlined_on_enclosing_model() => Some(rf),
                _ => None,
            })
            .collect();

        for rf in inlined_rfs {
            for field_name in rf.relation_info.fields.iter() {
                let field = self
                    .all
                    .iter()
                    .find(|f| matches!(f, Field::Scalar(sf) if &sf.name == field_name))
                    .expect("Expected inlined relation field reference to be an existing scalar field.");

                if let Field::Scalar(sf) = field {
                    if sf.read_only.set(true).is_err() {
                        // Ignore error
                    };
                }
            }
        }
    }

    pub fn id(&self) -> Option<&PrimaryKey> {
        self.primary_key.as_ref()
    }

    pub fn compound_id(&self) -> Option<&PrimaryKey> {
        if self
            .primary_key
            .as_ref()
            .map(|pk| pk.fields().len() > 1)
            .unwrap_or(false)
        {
            self.primary_key.as_ref()
        } else {
            None
        }
    }

    // Todo / WIP: Doesn't seem to exist anymore.
    // pub fn created_at(&self) -> &Option<ScalarFieldRef> {
    //     self.created_at.get_or_init(|| {
    //         self.scalar_weak()
    //             .iter()
    //             .map(|sf| sf.upgrade().unwrap())
    //             .find(|sf| sf.is_created_at())
    //     })
    // }

    pub fn updated_at(&self) -> &Option<ScalarFieldRef> {
        self.updated_at.get_or_init(|| {
            self.scalar_weak()
                .iter()
                .map(|sf| sf.upgrade().unwrap())
                .find(|sf| sf.is_updated_at)
        })
    }

    pub fn scalar(&self) -> Vec<ScalarFieldRef> {
        self.scalar_weak().iter().map(|f| f.upgrade().unwrap()).collect()
    }

    pub fn scalar_writable(&self) -> impl Iterator<Item = ScalarFieldRef> {
        self.scalar().into_iter().filter(|sf| !sf.is_read_only())
    }

    pub fn scalar_list(&self) -> Vec<ScalarFieldRef> {
        self.scalar().into_iter().filter(|sf| sf.is_list()).collect()
    }

    fn scalar_weak(&self) -> &[ScalarFieldWeak] {
        self.scalar
            .get_or_init(|| self.all.iter().fold(Vec::new(), Self::scalar_filter))
            .as_slice()
    }

    pub fn relation(&self) -> Vec<Arc<RelationField>> {
        self.relation_weak().iter().map(|f| f.upgrade().unwrap()).collect()
    }

    fn relation_weak(&self) -> &[Weak<RelationField>] {
        self.relation
            .get_or_init(|| self.all.iter().fold(Vec::new(), Self::relation_filter))
            .as_slice()
    }

    pub fn find_many_from_all(&self, names: &BTreeSet<String>) -> Vec<&Field> {
        self.all.iter().filter(|field| names.contains(field.name())).collect()
    }

    pub fn find_many_from_scalar(&self, names: &BTreeSet<String>) -> Vec<ScalarFieldRef> {
        self.scalar_weak()
            .iter()
            .filter(|field| names.contains(&field.upgrade().unwrap().name))
            .map(|field| field.upgrade().unwrap())
            .collect()
    }

    pub fn find_many_from_relation(&self, names: &BTreeSet<String>) -> Vec<Arc<RelationField>> {
        self.relation_weak()
            .iter()
            .filter(|field| names.contains(&field.upgrade().unwrap().name))
            .map(|field| field.upgrade().unwrap())
            .collect()
    }

    pub fn find_from_all(&self, name: &str) -> crate::Result<&Field> {
        self.all
            .iter()
            .find(|field| field.name() == name)
            .ok_or_else(|| DomainError::FieldNotFound {
                name: name.to_string(),
                model: self.model().name.clone(),
            })
    }

    pub fn find_from_scalar(&self, name: &str) -> crate::Result<ScalarFieldRef> {
        self.scalar_weak()
            .iter()
            .map(|field| field.upgrade().unwrap())
            .find(|field| field.name == name)
            .ok_or_else(|| DomainError::ScalarFieldNotFound {
                name: name.to_string(),
                model: self.model().name.clone(),
            })
    }

    fn model(&self) -> ModelRef {
        self.model.upgrade().unwrap()
    }

    pub fn find_from_relation_fields(&self, name: &str) -> crate::Result<Arc<RelationField>> {
        self.relation_weak()
            .iter()
            .map(|field| field.upgrade().unwrap())
            .find(|field| field.name == name)
            .ok_or_else(|| DomainError::RelationFieldNotFound {
                name: name.to_string(),
                model: self.model().name.clone(),
            })
    }

    pub fn find_from_relation(&self, name: &str, side: RelationSide) -> crate::Result<Arc<RelationField>> {
        self.relation_weak()
            .iter()
            .map(|field| field.upgrade().unwrap())
            .find(|field| field.relation().name == name && field.relation_side == side)
            .ok_or_else(|| DomainError::FieldForRelationNotFound {
                relation: name.to_string(),
                model: self.model().name.clone(),
            })
    }

    fn scalar_filter(mut acc: Vec<ScalarFieldWeak>, field: &Field) -> Vec<ScalarFieldWeak> {
        if let Field::Scalar(scalar_field) = field {
            acc.push(Arc::downgrade(scalar_field));
        };

        acc
    }

    fn relation_filter(mut acc: Vec<Weak<RelationField>>, field: &Field) -> Vec<Weak<RelationField>> {
        if let Field::Relation(relation_field) = field {
            acc.push(Arc::downgrade(relation_field));
        };

        acc
    }

    pub fn db_names(&self) -> impl Iterator<Item = String> + '_ {
        self.all
            .iter()
            .flat_map(|field| field.scalar_fields().into_iter().map(|f| f.db_name().to_owned()))
            .unique()
    }
}
