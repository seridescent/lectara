// @generated automatically by Diesel CLI.

diesel::table! {
    content_items (id) {
        id -> Integer,
        url -> Text,
        title -> Nullable<Text>,
        author -> Nullable<Text>,
        created_at -> Timestamp,
        body -> Nullable<Text>
    }
}
