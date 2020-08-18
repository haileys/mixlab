use std::borrow::Cow;

use shaderc::{Compiler, CompileOptions, ShaderKind};
use wgpu::ShaderModuleSource;

pub struct FragmentShader {
    spirv: Vec<u32>,
}

impl FragmentShader {
    pub fn compile(source: &str) -> Result<Self, shaderc::Error> {
        let mut compiler = Compiler::new().unwrap();
        let options = CompileOptions::new().unwrap();

        let result = compiler.compile_into_spirv(source, ShaderKind::Fragment, "fragment-shader", "main", Some(&options))?;

        Ok(FragmentShader {
            spirv: result.as_binary().to_vec(),
        })
    }

    pub fn module_source(&self) -> ShaderModuleSource<'_> {
        ShaderModuleSource::SpirV(Cow::Borrowed(&self.spirv))
    }
}

pub fn compile(filename: &str, source: &str, kind: ShaderKind) -> Vec<u32> {
    let mut compiler = Compiler::new().unwrap();
    let options = CompileOptions::new().unwrap();

    let result = compiler.compile_into_spirv(source, kind, filename, "main", Some(&options)).unwrap();

    result.as_binary().to_vec()
}
