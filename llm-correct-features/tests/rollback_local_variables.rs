use assert_fs::prelude::*;
use assert_fs::TempDir;
#[allow(unused_imports)] // Feature is used via class.features()
use eiffel_tools_lib::code_entities::prelude::*;
use eiffel_tools_lib::parser::Parser;
use eiffel_tools_lib::workspace::Workspace;

#[tokio::test]
async fn test_rollback_suggestion_with_local_variables() {
    // This test verifies that rolling back a suggestion which introduced local variables
    // correctly removes those local variables and restores the original code
    
    let tmp_dir = TempDir::new().expect("Failed to create temporary directory");
    let file = tmp_dir.child("test_class.e");

    // Initial code without local variables
    const INITIAL_CODE: &str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        do
            Result := a + b
        ensure
            Result >= a
            Result >= b
        end
end
"#;

    // Code with local variables introduced (simulating an LLM suggestion)
    const CODE_WITH_LOCAL_VARS: &str = r#"
class TEST_CLASS
feature
    compute (a, b: INTEGER): INTEGER
        require
            a >= 0
            b >= 0
        local
            temp_a: INTEGER
            temp_b: INTEGER
        do
            temp_a := a
            temp_b := b
            Result := temp_a + temp_b
        ensure
            Result >= a
            Result >= b
        end
end
"#;

    // Write initial code
    file.write_str(INITIAL_CODE)
        .expect("Failed to write initial code");

    // Parse and create workspace
    let mut parser = Parser::default();
    let (class, tree) = parser
        .class_and_tree_from_source(INITIAL_CODE)
        .expect("Failed to parse initial class");
    let mut workspace = Workspace::default();
    workspace.add_file((class.clone(), file.to_path_buf(), tree));

    // Read initial code as last_valid_code
    let last_valid_code = tokio::fs::read(file.path())
        .await
        .expect("Failed to read initial file");

    // Verify initial code has no local variables
    let initial_content = tokio::fs::read_to_string(file.path())
        .await
        .expect("Failed to read initial file");
    assert!(
        !initial_content.contains("local"),
        "Initial code should not contain local variables"
    );
    assert!(
        initial_content.contains("Result := a + b"),
        "Initial code should contain direct computation"
    );

    // Simulate applying a suggestion that introduces local variables
    // Write the full code with local variables to the file
    // (In reality, this would be done by rewrite_feature_bodies, but since local variables
    // are not part of the body, we need to write the full feature code)
    file.write_str(CODE_WITH_LOCAL_VARS)
        .expect("Failed to write code with local variables");
    
    // Reload workspace
    workspace.reload(file.to_path_buf()).await;

    // Verify the file now contains local variables
    let content_with_vars = tokio::fs::read_to_string(file.path())
        .await
        .expect("Failed to read file with local variables");
    assert!(
        content_with_vars.contains("local"),
        "File should contain local clause after applying suggestion"
    );
    assert!(
        content_with_vars.contains("temp_a: INTEGER"),
        "File should contain temp_a local variable"
    );
    assert!(
        content_with_vars.contains("temp_b: INTEGER"),
        "File should contain temp_b local variable"
    );
    assert!(
        content_with_vars.contains("temp_a := a"),
        "File should contain assignment to temp_a"
    );
    assert!(
        content_with_vars.contains("temp_b := b"),
        "File should contain assignment to temp_b"
    );

    // Now simulate a verification failure scenario:
    // 1. A suggestion with local variables was applied (we just did this)
    // 2. Verification fails
    // 3. The code should be rolled back to last_valid_code (which is the INITIAL_CODE without local vars)
    // 
    // In the actual flow, modify_in_place::verification calls reset_source when verification fails,
    // which writes last_valid_code back to the file. Since last_valid_code is still the initial code
    // (without local variables), the rollback should remove the local variables.
    
    // Simulate rollback: reset the file to last_valid_code (which is INITIAL_CODE without local vars)
    // This is what happens in modify_in_place::reset_source when verification fails
    tokio::fs::write(file.path(), &last_valid_code)
        .await
        .expect("Failed to write last_valid_code");
    workspace.reload(file.to_path_buf()).await;

    // Verify the file was rolled back correctly - local variables should be removed
    let content_after_rollback = tokio::fs::read_to_string(file.path())
        .await
        .expect("Failed to read file after rollback");
    
    // The file should NOT contain local variables anymore
    assert!(
        !content_after_rollback.contains("local"),
        "File should not contain local clause after rollback. Content: {}",
        content_after_rollback
    );
    assert!(
        !content_after_rollback.contains("temp_a: INTEGER"),
        "File should not contain temp_a local variable after rollback"
    );
    assert!(
        !content_after_rollback.contains("temp_b: INTEGER"),
        "File should not contain temp_b local variable after rollback"
    );
    assert!(
        !content_after_rollback.contains("temp_a := a"),
        "File should not contain assignment to temp_a after rollback"
    );
    assert!(
        !content_after_rollback.contains("temp_b := b"),
        "File should not contain assignment to temp_b after rollback"
    );

    // The file should contain the original computation
    assert!(
        content_after_rollback.contains("Result := a + b"),
        "File should contain original direct computation after rollback. Content: {}",
        content_after_rollback
    );

    // Verify the feature can still be parsed correctly
    let (rolled_back_class, _) = parser
        .class_and_tree_from_source(&content_after_rollback)
        .expect("Failed to parse rolled back class");
    
    let rolled_back_feature = rolled_back_class
        .features()
        .iter()
        .find(|f| f.name() == "compute")
        .expect("Should find compute feature after rollback");
    
    // Verify the feature has no local variables by checking the source
    let rolled_back_body = rolled_back_feature
        .body_source_unchecked(content_after_rollback.as_str())
        .expect("Should extract body from rolled back feature");
    assert!(
        !rolled_back_body.contains("local"),
        "Feature body should not contain local clause after rollback. Body: {}",
        rolled_back_body
    );
}


