use astd::extract_function_details;

#[test]
fn test_simple_function() {
    let source = "int my_function(int a, float b);";
    let extracted = extract_function_details(source);
    assert_eq!(extracted.len(), 1);
    let (prefix, ret, name) = &extracted[0];
    assert_eq!(prefix, "");
    assert_eq!(ret, "int");
    assert_eq!(name, "my_function");
}

#[test]
fn test_function_with_template() {
    let source = "template <typename T> T func_template(T a);";
    let extracted = extract_function_details(source);
    assert_eq!(extracted.len(), 1);
    let (prefix, ret, name) = &extracted[0];
    assert_eq!(prefix, "template <typename T>");
    assert_eq!(ret, "T");
    assert_eq!(name, "func_template");
}

#[test]
fn test_functions_with_comments() {
    let source = r#"
        // This function does something
        int sum(int a, int b);

        /* Multi-line comment describing the function */
        double average(double a, double b);
    "#;
    let extracted = extract_function_details(source);
    assert_eq!(extracted.len(), 2);
    let (_, ret1, name1) = &extracted[0];
    assert_eq!(ret1, "int");
    assert_eq!(name1, "sum");
    let (_, ret2, name2) = &extracted[1];
    assert_eq!(ret2, "double");
    assert_eq!(name2, "average");
}

#[test]
fn test_complex_signature() {
    let source = "const std::vector<int>& get_vector() const;";
    let extracted = extract_function_details(source);
    assert_eq!(extracted.len(), 1);
    let (_, ret, name) = &extracted[0];
    assert_eq!(ret, "const std::vector<int>&");
    assert_eq!(name, "get_vector");
}
