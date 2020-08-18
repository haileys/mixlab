pub fn fragment(source: &str) -> Vec<u32> {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();

    let result = compiler.compile_into_spirv(
        source, shaderc::ShaderKind::Fragment,
        "shader.glsl", "main", Some(&options)).unwrap();

    result.as_binary().to_vec()
}
