use std::path::PathBuf;

use quote::{ToTokens, quote};
use syn::{Path, parse::{Parse, ParseStream}, parse_macro_input, spanned::Spanned};
use wesl::{Mangler, Resolver};
use proc_macro_error2::{OptionExt, ResultExt, abort};

struct ShaderPath {
    // used for validation
    path: Path,
}

impl Parse for ShaderPath {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ShaderPath { path: input.call(Path::parse_mod_style)? })
    }
}

// todo use trybuild to test errors, see: https://docs.rs/trybuild/latest/trybuild/index.html
/// Include a WGSL file compiled with `wesl_build` as a string.
///
/// The argument corresponds to the shaders path from your shader root dir
///
/// ## Example
/// ```
/// use wesl_build_import::include_wesl;
///
/// // ok
/// include_wesl!(test_mod::test_mod_file);
/// // err: path to module is already based on root(package)
/// include_wesl!(package::test_mod::test_mod_file);
/// // err: module not a shader
/// include_wesl!(test_mod);
/// // err: no such file
/// include_wesl!(green_screen::cutout);
/// ```
#[proc_macro_error2::proc_macro_error]
#[proc_macro]
pub fn include_wesl(shader_path: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path_str = shader_path.to_string();
    // validate
    let shader_path = parse_macro_input!(shader_path as ShaderPath);
    let Some(path_first) = shader_path.path.segments.first() else {
        proc_macro_error2::abort_call_site!("the shader import path must be non-empty");
    };

    let path_last = shader_path.path.segments.last().expect("checked above");

    // error if `package` path prefix is present
    if path_first.ident == "package" {
        abort!(
            path_first.ident.span(),
            "path to module is already based on root(package) so there is no need to specify it"
        );
    }

    let mod_path = wesl::ModulePath::new(
        wesl::syntax::PathOrigin::Absolute,
        path_str
            .split("::")
            .map(|str| str.to_owned())
            .collect::<Vec<_>>(),
    );
    let path_last_name = &path_last.into_token_stream().to_string();

    // validate file exists and 
    {
        // use shader_root dir from WESL_BUILD_DIR_ROOT_PATH to find shader_path
        // use span of part of path with error
        let mut shader_dir: PathBuf = std::env::var_os("WESL_BUILD_DIR_ROOT_PATH")
            .expect_or_abort("`wesl_build::build_shader_dir` must be run first, to set the WESL_BUILD_DIR_ROOT_PATH environment variable")
            .into();
        shader_dir.extend(&mod_path.components);

        let shader_exists = shader_exists(&mut shader_dir);
        // depth first dir search, to find error point, like is mod not file or so such file
        if !shader_exists {
            if let Ok(dir_metadata) = std::fs::metadata(&shader_dir) && dir_metadata.is_dir() {
                abort!(path_last.ident.span(),
                    "`{}` is a module not a shader file", &path_last_name;
                    help = "add `::` and the shader in the modules name"
                )
            }

            shader_dir.pop();
            let mut component_idx = mod_path.components.len() - 2;

            loop {
                let dir_exists = std::fs::exists(&shader_dir).ok().is_some_and(|b| b);
                if dir_exists || component_idx == 0 {
                    proc_macro_error2::emit_error!(shader_path.path.segments[component_idx].span(), ""; note = "this is the last component of the path that exists");
                    abort!(proc_macro_error2::SpanRange {
                            // span just the not found part of the path
                            first: shader_path.path.segments[component_idx].ident.span().unwrap().end().into(),
                            last: path_last.ident.span(),
                        },
                        "shader `{}` does not exist", &path_last_name
                    )
                } else {
                    if !shader_dir.pop() {
                        break;
                    }
                    component_idx -= 1;
                }
            }
        }
    }

    // !! keep in sync with mangler used in wesl_build !!
    let name_mangler = wesl::EscapeMangler;
    // mange name
    let shader_path = name_mangler.mangle(&mod_path, &path_last_name);

    // output is the same as calling [`wasl::include_wesl!`]
    quote! {
        include_str!(concat!(env!("OUT_DIR"), "/", #shader_path, ".wgsl"))
    }.into()
}

fn shader_exists(shader_dir: &mut PathBuf) -> bool {
    shader_dir.set_extension("wesl");
    let exists = std::fs::exists(&shader_dir).ok().is_some_and(|b| b)
    || {
        shader_dir.set_extension("wgsl");
        std::fs::exists(&shader_dir).ok().is_some_and(|b| b)
    };

    // reset extension
    shader_dir.set_extension("");

    exists
}
