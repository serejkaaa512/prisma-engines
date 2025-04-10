use crate::constants::json_null;

use super::*;
use constants::{aggregations, filters};
use datamodel_connector::ConnectorCapability;
use prisma_models::{dml::DefaultValue, PrismaValue};

/// Builds filter types for the given model field.
#[tracing::instrument(skip(ctx, field, include_aggregates))]
pub(crate) fn get_field_filter_types(
    ctx: &mut BuilderContext,
    field: &ModelField,
    include_aggregates: bool,
) -> Vec<InputType> {
    match field {
        ModelField::Relation(rf) => {
            let mut types = vec![InputType::object(full_relation_filter(ctx, rf))];
            types.extend(mto1_relation_filter_shorthand_types(ctx, rf));
            types
        }
        ModelField::Composite(_) => vec![], // [Composites] todo
        ModelField::Scalar(sf) if field.is_list() => vec![InputType::object(scalar_list_filter_type(ctx, sf))],
        ModelField::Scalar(sf) => {
            let mut types = vec![InputType::object(full_scalar_filter_type(
                ctx,
                &sf.type_identifier,
                sf.is_list(),
                !sf.is_required(),
                false,
                include_aggregates,
            ))];

            if sf.type_identifier != TypeIdentifier::Json {
                types.push(map_scalar_input_type_for_field(ctx, sf)); // Scalar equality shorthand

                if !sf.is_required() {
                    types.push(InputType::null()); // Scalar null-equality shorthand
                }
            }

            types
        }
    }
}

/// Builds shorthand relation equality (`is`) filter for to-one: `where: { relation_field: { ... } }` (no `is` in between).
/// If the field is also not required, null is also added as possible type.
#[tracing::instrument(skip(ctx, rf))]
fn mto1_relation_filter_shorthand_types(ctx: &mut BuilderContext, rf: &RelationFieldRef) -> Vec<InputType> {
    let mut types = vec![];

    if !rf.is_list() {
        let related_model = rf.related_model();
        let related_input_type = filter_objects::where_object_type(ctx, &related_model);
        types.push(InputType::object(related_input_type));

        if !rf.is_required() {
            types.push(InputType::null());
        }
    }

    types
}

#[tracing::instrument(skip(ctx, rf))]
fn full_relation_filter(ctx: &mut BuilderContext, rf: &RelationFieldRef) -> InputObjectTypeWeakRef {
    let related_model = rf.related_model();
    let related_input_type = filter_objects::where_object_type(ctx, &related_model);
    let list = if rf.is_list() { "List" } else { "" };
    let ident = Identifier::new(
        format!("{}{}RelationFilter", capitalize(&related_model.name), list),
        PRISMA_NAMESPACE,
    );

    return_cached_input!(ctx, &ident);
    let object = Arc::new(init_input_object_type(ident.clone()));
    ctx.cache_input_type(ident, object.clone());

    let fields = if rf.is_list() {
        vec![
            input_field(filters::EVERY, InputType::object(related_input_type.clone()), None).optional(),
            input_field(filters::SOME, InputType::object(related_input_type.clone()), None).optional(),
            input_field(filters::NONE, InputType::object(related_input_type), None).optional(),
        ]
    } else {
        vec![
            input_field(filters::IS, InputType::object(related_input_type.clone()), None)
                .optional()
                .nullable_if(!rf.is_required()),
            input_field(filters::IS_NOT, InputType::object(related_input_type), None)
                .optional()
                .nullable_if(!rf.is_required()),
        ]
    };

    object.set_fields(fields);
    Arc::downgrade(&object)
}

#[tracing::instrument(skip(ctx, sf))]
fn scalar_list_filter_type(ctx: &mut BuilderContext, sf: &ScalarFieldRef) -> InputObjectTypeWeakRef {
    let ident = Identifier::new(
        scalar_filter_name(&sf.type_identifier, true, !sf.is_required(), false, false),
        PRISMA_NAMESPACE,
    );
    return_cached_input!(ctx, &ident);

    let mut object = init_input_object_type(ident.clone());
    object.require_exactly_one_field();

    let object = Arc::new(object);
    ctx.cache_input_type(ident, object.clone());

    let mapped_nonlist_type = map_scalar_input_type(ctx, &sf.type_identifier, false);
    let mapped_list_type = InputType::list(mapped_nonlist_type.clone());
    let mut fields: Vec<_> = equality_filters(mapped_list_type.clone(), !sf.is_required()).collect();

    fields.push(
        input_field(filters::HAS, mapped_nonlist_type, None)
            .optional()
            .nullable_if(!sf.is_required()),
    );

    fields.push(input_field(filters::HAS_EVERY, mapped_list_type.clone(), None).optional());
    fields.push(input_field(filters::HAS_SOME, mapped_list_type, None).optional());
    fields.push(input_field(filters::IS_EMPTY, InputType::boolean(), None).optional());

    object.set_fields(fields);
    Arc::downgrade(&object)
}

#[tracing::instrument(skip(ctx, typ, list, nullable, nested, include_aggregates))]
fn full_scalar_filter_type(
    ctx: &mut BuilderContext,
    typ: &TypeIdentifier,
    list: bool,
    nullable: bool,
    nested: bool,
    include_aggregates: bool,
) -> InputObjectTypeWeakRef {
    let ident = Identifier::new(
        scalar_filter_name(typ, list, nullable, nested, include_aggregates),
        PRISMA_NAMESPACE,
    );

    return_cached_input!(ctx, &ident);

    let object = Arc::new(init_input_object_type(ident.clone()));
    ctx.cache_input_type(ident, object.clone());

    let mapped_scalar_type = map_scalar_input_type(ctx, typ, list);

    let mut fields: Vec<_> = match typ {
        TypeIdentifier::String | TypeIdentifier::UUID => equality_filters(mapped_scalar_type.clone(), nullable)
            .chain(inclusion_filters(mapped_scalar_type.clone(), nullable))
            .chain(alphanumeric_filters(mapped_scalar_type.clone()))
            .chain(string_filters(ctx, mapped_scalar_type.clone()))
            .chain(query_mode_field(ctx, nested))
            .collect(),

        TypeIdentifier::Int
        | TypeIdentifier::BigInt
        | TypeIdentifier::Float
        | TypeIdentifier::DateTime
        | TypeIdentifier::Decimal => equality_filters(mapped_scalar_type.clone(), nullable)
            .chain(inclusion_filters(mapped_scalar_type.clone(), nullable))
            .chain(alphanumeric_filters(mapped_scalar_type.clone()))
            .collect(),

        TypeIdentifier::Json => {
            if ctx.has_feature(&PreviewFeature::FilterJson)
                && (ctx.capabilities.contains(ConnectorCapability::JsonFilteringJsonPath)
                    || ctx.capabilities.contains(ConnectorCapability::JsonFilteringArrayPath))
            {
                json_equality_filters(ctx, mapped_scalar_type.clone(), nullable)
                    .chain(alphanumeric_filters(mapped_scalar_type.clone()))
                    .chain(json_filters(ctx))
                    .collect()
            } else {
                json_equality_filters(ctx, mapped_scalar_type.clone(), nullable).collect()
            }
        }

        TypeIdentifier::Boolean | TypeIdentifier::Xml | TypeIdentifier::Bytes => {
            equality_filters(mapped_scalar_type.clone(), nullable).collect()
        }

        TypeIdentifier::Enum(_) => equality_filters(mapped_scalar_type.clone(), nullable)
            .chain(inclusion_filters(mapped_scalar_type.clone(), nullable))
            .collect(),

        TypeIdentifier::Unsupported => unreachable!("No unsupported field should reach that path"),
    };

    fields.push(not_filter_field(
        ctx,
        typ,
        mapped_scalar_type,
        nullable,
        include_aggregates,
        list,
    ));

    if include_aggregates {
        fields.push(aggregate_filter_field(
            ctx,
            aggregations::UNDERSCORE_COUNT,
            &TypeIdentifier::Int,
            nullable,
            list,
        ));

        if typ.is_numeric() {
            let avg_type = map_avg_type_ident(typ.clone());
            fields.push(aggregate_filter_field(
                ctx,
                aggregations::UNDERSCORE_AVG,
                &avg_type,
                nullable,
                list,
            ));

            fields.push(aggregate_filter_field(
                ctx,
                aggregations::UNDERSCORE_SUM,
                typ,
                nullable,
                list,
            ));
        }

        if !list {
            fields.push(aggregate_filter_field(
                ctx,
                aggregations::UNDERSCORE_MIN,
                typ,
                nullable,
                list,
            ));

            fields.push(aggregate_filter_field(
                ctx,
                aggregations::UNDERSCORE_MAX,
                typ,
                nullable,
                list,
            ));
        }
    }

    object.set_fields(fields);
    Arc::downgrade(&object)
}

fn equality_filters(mapped_type: InputType, nullable: bool) -> impl Iterator<Item = InputField> {
    std::iter::once(
        input_field(filters::EQUALS, mapped_type, None)
            .optional()
            .nullable_if(nullable),
    )
}

fn json_equality_filters(
    ctx: &BuilderContext,
    mapped_type: InputType,
    nullable: bool,
) -> impl Iterator<Item = InputField> {
    let field = if ctx.has_capability(ConnectorCapability::AdvancedJsonNullability) {
        let enum_type = json_null_filter_enum();
        input_field(filters::EQUALS, vec![InputType::Enum(enum_type), mapped_type], None).optional()
    } else {
        input_field(filters::EQUALS, vec![mapped_type], None)
            .optional()
            .nullable_if(nullable)
    };

    std::iter::once(field)
}

fn json_null_filter_enum() -> EnumTypeRef {
    Arc::new(string_enum_type(
        json_null::FILTER_ENUM_NAME,
        vec![
            json_null::DB_NULL.to_owned(),
            json_null::JSON_NULL.to_owned(),
            json_null::ANY_NULL.to_owned(),
        ],
    ))
}

fn inclusion_filters(mapped_type: InputType, nullable: bool) -> impl Iterator<Item = InputField> {
    let typ = InputType::list(mapped_type);

    vec![
        input_field(filters::IN, typ.clone(), None)
            .optional()
            .nullable_if(nullable),
        input_field(filters::NOT_IN, typ, None).optional().nullable_if(nullable), // Kept for legacy reasons!
    ]
    .into_iter()
}

fn alphanumeric_filters(mapped_type: InputType) -> impl Iterator<Item = InputField> {
    vec![
        input_field(filters::LOWER_THAN, mapped_type.clone(), None).optional(),
        input_field(filters::LOWER_THAN_OR_EQUAL, mapped_type.clone(), None).optional(),
        input_field(filters::GREATER_THAN, mapped_type.clone(), None).optional(),
        input_field(filters::GREATER_THAN_OR_EQUAL, mapped_type, None).optional(),
    ]
    .into_iter()
}

fn string_filters(ctx: &mut BuilderContext, mapped_type: InputType) -> impl Iterator<Item = InputField> {
    let mut string_filters = vec![
        input_field(filters::CONTAINS, mapped_type.clone(), None).optional(),
        input_field(filters::STARTS_WITH, mapped_type.clone(), None).optional(),
        input_field(filters::ENDS_WITH, mapped_type.clone(), None).optional(),
    ];

    if ctx.has_feature(&PreviewFeature::FullTextSearch)
        && ctx.has_capability(ConnectorCapability::FullTextSearchWithoutIndex)
    {
        string_filters.push(input_field(filters::SEARCH, mapped_type, None).optional());
    }

    string_filters.into_iter()
}

fn json_filters(ctx: &mut BuilderContext) -> impl Iterator<Item = InputField> {
    // TODO: also add json-specific "keys" filters
    // TODO: add json_type filter
    let path_type = if ctx.capabilities.contains(ConnectorCapability::JsonFilteringJsonPath) {
        InputType::string()
    } else if ctx.capabilities.contains(ConnectorCapability::JsonFilteringArrayPath) {
        InputType::list(InputType::string())
    } else {
        unreachable!()
    };

    vec![
        input_field(filters::PATH, vec![path_type], None).optional(),
        input_field(filters::STRING_CONTAINS, InputType::string(), None).optional(),
        input_field(filters::STRING_STARTS_WITH, InputType::string(), None).optional(),
        input_field(filters::STRING_ENDS_WITH, InputType::string(), None).optional(),
        input_field(filters::ARRAY_CONTAINS, InputType::json(), None)
            .optional()
            .nullable(),
        input_field(filters::ARRAY_STARTS_WITH, InputType::json(), None)
            .optional()
            .nullable(),
        input_field(filters::ARRAY_ENDS_WITH, InputType::json(), None)
            .optional()
            .nullable(),
    ]
    .into_iter()
}

fn query_mode_field(ctx: &BuilderContext, nested: bool) -> impl Iterator<Item = InputField> {
    // Limit query mode field to the topmost filter level.
    // Only build mode field for connectors with insensitive filter support.
    let fields = if !nested && ctx.capabilities.contains(ConnectorCapability::InsensitiveFilters) {
        let enum_type = Arc::new(string_enum_type(
            "QueryMode",
            vec![filters::DEFAULT.to_owned(), filters::INSENSITIVE.to_owned()],
        ));

        let field = input_field(
            filters::MODE,
            InputType::enum_type(enum_type),
            Some(DefaultValue::new_single(PrismaValue::Enum(filters::DEFAULT.to_owned()))),
        )
        .optional();

        vec![field]
    } else {
        vec![]
    };

    fields.into_iter()
}

fn scalar_filter_name(
    typ: &TypeIdentifier,
    list: bool,
    nullable: bool,
    nested: bool,
    include_aggregates: bool,
) -> String {
    let list = if list { "List" } else { "" };
    let nullable = if nullable { "Nullable" } else { "" };
    let nested = if nested { "Nested" } else { "" };
    let aggregates = if include_aggregates { "WithAggregates" } else { "" };

    match typ {
        TypeIdentifier::UUID => format!("{}Uuid{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::String => format!("{}String{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Int => format!("{}Int{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::BigInt => format!("{}BigInt{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Float => format!("{}Float{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Decimal => format!("{}Decimal{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Boolean => format!("{}Bool{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::DateTime => format!("{}DateTime{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Json => format!("{}Json{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Enum(ref e) => format!("{}Enum{}{}{}{}Filter", nested, e, nullable, list, aggregates),
        TypeIdentifier::Xml => format!("{}Xml{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Bytes => format!("{}Bytes{}{}{}Filter", nested, nullable, list, aggregates),
        TypeIdentifier::Unsupported => unreachable!("No unsupported field should reach that path"),
    }
}

fn aggregate_filter_field(
    ctx: &mut BuilderContext,
    aggregation: &str,
    typ: &TypeIdentifier,
    nullable: bool,
    list: bool,
) -> InputField {
    let filters = full_scalar_filter_type(ctx, typ, list, nullable, true, false);
    input_field(aggregation, InputType::object(filters), None).optional()
}

fn map_avg_type_ident(typ: TypeIdentifier) -> TypeIdentifier {
    match &typ {
        TypeIdentifier::Int | TypeIdentifier::BigInt | TypeIdentifier::Float => TypeIdentifier::Float,
        _ => typ,
    }
}

// Shorthand `not equals` filter input field, skips the nested object filter.
fn not_filter_field(
    ctx: &mut BuilderContext,
    typ: &TypeIdentifier,
    mapped_scalar_type: InputType,
    is_nullable: bool,
    include_aggregates: bool,
    is_list: bool,
) -> InputField {
    let has_adv_json = ctx.has_capability(ConnectorCapability::AdvancedJsonNullability);

    match typ {
        // Json is not nullable on dbs with `AdvancedJsonNullability`, only by proxy through an enum.
        TypeIdentifier::Json if has_adv_json => {
            let enum_type = json_null_filter_enum();

            input_field(
                filters::NOT_LOWERCASE,
                vec![InputType::Enum(enum_type), mapped_scalar_type],
                None,
            )
            .optional()
        }

        TypeIdentifier::Json => input_field(filters::NOT_LOWERCASE, vec![mapped_scalar_type], None)
            .optional()
            .nullable_if(is_nullable),

        _ => {
            // Full nested filter. Only available on non-JSON fields.
            let shorthand = InputType::object(full_scalar_filter_type(
                ctx,
                typ,
                is_list,
                is_nullable,
                true,
                include_aggregates,
            ));

            input_field(filters::NOT_LOWERCASE, vec![mapped_scalar_type, shorthand], None)
                .optional()
                .nullable_if(is_nullable)
        }
    }
}
