use shaderc::{Compiler, CompileOptions, ShaderKind};

pub fn vertex(filename: &str, source: &str) -> Vec<u32> {
    compile(filename, source, ShaderKind::Vertex)
}

pub fn fragment(filename: &str, source: &str) -> Vec<u32> {
    compile(filename, source, ShaderKind::Fragment)
}

fn compile(filename: &str, source: &str, kind: ShaderKind) -> Vec<u32> {
    let mut compiler = Compiler::new().unwrap();
    let options = CompileOptions::new().unwrap();

    let result = compiler.compile_into_spirv(source, kind, filename, "main", Some(&options)).unwrap();

    result.as_binary().to_vec()
}
