pub static MIGRATIONS: &'static [(i64, &'static str)] = &[
    (0, include_str!("migrations/0_init.sql")),
    (20200804, include_str!("migrations/20200804_create_media_tables.sql")),
    (20200805, include_str!("migrations/20200805_create_workspace_table.sql")),
];
