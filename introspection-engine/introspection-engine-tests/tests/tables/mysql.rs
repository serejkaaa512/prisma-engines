use indoc::indoc;
use introspection_engine_tests::test_api::*;

#[test_connector(tags(Mysql))]
async fn a_table_with_non_id_autoincrement(api: &TestApi) -> TestResult {
    let setup = r#"
        CREATE TABLE `Test` (
            `id` INTEGER PRIMARY KEY,
            `authorId` INTEGER AUTO_INCREMENT UNIQUE
        );
    "#;

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model Test {
          id       Int @id
          authorId Int @unique(map: "authorId") @default(autoincrement())
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}

#[test_connector(tags(Mysql8), preview_features("extendedIndexes"))]
async fn a_table_with_length_prefixed_primary_key(api: &TestApi) -> TestResult {
    let setup = indoc! {r#"
        CREATE TABLE `A` (
            `id` TEXT NOT NULL,
            CONSTRAINT A_id_pkey PRIMARY KEY (id(30))
        )
    "#};

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model A {
          id String @id(length: 30) @db.Text
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}

#[test_connector(tags(Mysql8), preview_features("extendedIndexes"))]
async fn a_table_with_length_prefixed_unique(api: &TestApi) -> TestResult {
    let setup = indoc! {r#"
        CREATE TABLE `A` (
            `id` INT  PRIMARY KEY,
            `a`  TEXT NOT NULL,
            CONSTRAINT A_a_key UNIQUE (a(30))
        )
    "#};

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model A {
          id Int    @id
          a  String @unique(length: 30) @db.Text
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}

#[test_connector(tags(Mysql8), preview_features("extendedIndexes"))]
async fn a_table_with_length_prefixed_compound_unique(api: &TestApi) -> TestResult {
    let setup = indoc! {r#"
        CREATE TABLE `A` (
            `id` INT  PRIMARY KEY,
            `a`  TEXT NOT NULL,
            `b`  TEXT NOT NULL,
            CONSTRAINT A_a_b_key UNIQUE (a(30), b(20))
        )
    "#};

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model A {
          id Int    @id
          a  String @db.Text
          b  String @db.Text

          @@unique([a(length: 30), b(length: 20)])
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}

#[test_connector(tags(Mysql8), preview_features("extendedIndexes"))]
async fn a_table_with_length_prefixed_index(api: &TestApi) -> TestResult {
    let setup = indoc! {r#"
        CREATE TABLE `A` (
            `id` INT  PRIMARY KEY,
            `a`  TEXT NOT NULL,
            `b`  TEXT NOT NULL
        );
        
        CREATE INDEX A_a_b_idx ON `A` (a(30), b(20));
    "#};

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model A {
          id Int    @id
          a  String @db.Text
          b  String @db.Text

          @@index([a(length: 30), b(length: 20)])
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}

#[test_connector(tags(Mysql8), preview_features("extendedIndexes"))]
async fn a_table_with_descending_index(api: &TestApi) -> TestResult {
    let setup = indoc! {r#"
        CREATE TABLE `A` (
            `id` INT  PRIMARY KEY,
            `a`  INT NOT NULL,
            `b`  INT NOT NULL
        );
        
        CREATE INDEX A_a_b_idx ON `A` (a ASC, b DESC);
    "#};

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model A {
          id Int @id
          a  Int
          b  Int

          @@index([a, b(sort: Desc)])
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}

#[test_connector(tags(Mysql8), preview_features("extendedIndexes"))]
async fn a_table_with_descending_unique(api: &TestApi) -> TestResult {
    let setup = indoc! {r#"
        CREATE TABLE `A` (
            `id` INT  PRIMARY KEY,
            `a`  INT NOT NULL,
            `b`  INT NOT NULL
        );
        
        CREATE UNIQUE INDEX A_a_b_key ON `A` (a ASC, b DESC);
    "#};

    api.raw_cmd(setup).await;

    let expected = expect![[r#"
        model A {
          id Int @id
          a  Int
          b  Int

          @@unique([a, b(sort: Desc)])
        }
    "#]];

    expected.assert_eq(&api.introspect_dml().await?);

    Ok(())
}
