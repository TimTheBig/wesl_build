// TODO TEST <-!->!!=!!=!!=!!=!!<-!->
#![cfg(feature = "math_consts_ext")]

use std::{path::Path};

use wesl::{BasicSourceMap, ModulePath, StandardResolver};
use wgsl_types::inst::LiteralInstance;

use crate::WeslBuildExtension;

macro_rules! type_const {
    ($type:ty; $const_name:ident) => {
        (
            concat!(stringify!($type), "_", stringify!($const_name)).to_uppercase(),
            ::wgsl_types::inst::LiteralInstance::from(<$type>::$const_name)
        )
    };
    ($($($types:ty),+; $const_name:ident),+$(,)?) => {
        [
            $($(
                type_const!($types; $const_name),
            )+)+
        ]
    };
}

macro_rules! abstract_const {
    ($const_name:expr, $const_val:expr) => {
        ($const_name.to_string(), LiteralInstance::AbstractFloat($const_val))
    };
}

/// Adds f32, f64, u32, and i32 constants as well as AbstractFloat(f64) mathematical constants
///
/// This allows there use in any of your WESL files, under the `constants` module
///
/// ### Example
/// ```wesl
/// import constants::{PI, E, TAU, SQRT_2, U32_MAX, I32_MIN};
///
/// const WACKY_NUMBER = PI / E + TAU;
/// const EXTRA_WACKY_NUMBER = U32_MAX >> u32(f32(I32_MIN) / SQRT_2);
/// ```
///
/// ### Naming
/// - All there names are in SCREAMING_SNAKE_CASE
/// - Type specific constants are pre-fixed with the type name (eg. `F32_MIN`, `F64_EPSILON`)
pub struct MathConstantsExtension;

impl WeslBuildExtension<StandardResolver> for MathConstantsExtension {
    fn name<'n>(&self) -> std::borrow::Cow<'n, str> {
        "MathConstantsExtension".into()
    }

    fn init_root(
        &mut self,
        _shader_path: &str,
        res: &mut wesl::Wesl<StandardResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use core::f64::consts as f64_consts;
        use hyperreal::{Real, Rational};

        // type based consts
        res.add_constants(
            // todo add naga_ext feature that adds f64 consts here
            type_const!(
                u32, i32, f32; MAX,
                u32, i32, f32; MIN,
                f32; MIN_POSITIVE,
                f32; EPSILON
            ).into_iter()
        );
        // math consts, all as f64 for AbstractFloat precision in consts
        res.add_constants([
            abstract_const!("SQRT_2", Real::from(2).sqrt().unwrap()),
            abstract_const!("INV_SQRT_2", Real::from(2).sqrt().unwrap().inverse(/* not over 0 */).unwrap()),
            abstract_const!("E", Real::e()),

            abstract_const!("PI", Real::pi()),
            abstract_const!("TAU", Real::tau()),
            abstract_const!("INV_PI", Real::pi().inverse().unwrap(/* not over 0 */)),
            abstract_const!("INV_TAU", Real::tau().inverse().unwrap(/* not over 0 */)),

            abstract_const!("DEG_TO_RAD", Real::one().to_radians()),
            abstract_const!("RAD_TO_DEG", Real::one().to_degrees()),

            abstract_const!("FRAC_PI_2", (Real::pi() / Real::from(2)).unwrap(/* not over 0 */)),
            abstract_const!("FRAC_PI_3", (Real::pi() / Real::from(3)).unwrap()),
            abstract_const!("FRAC_PI_4", (Real::pi() / Real::from(4)).unwrap()),
            abstract_const!("FRAC_PI_6", (Real::pi() / Real::from(6)).unwrap()),
            abstract_const!("FRAC_PI_8", (Real::pi() / Real::from(8)).unwrap()),

            abstract_const!("LOG2_E", Real::e().log2().unwrap()),
            abstract_const!("LOG2_10", Real::from(10).log2().unwrap()),
            abstract_const!("LOG10_2", Real::from(2).log10().unwrap()),

            // unstable feature: "more_float_constants"
            // abstract_const!(PHI),
            // abstract_const!(EGAMMA),
        ]);

        Ok(())
    }

    fn enter_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    fn exit_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    fn post_build(
        &mut self,
        _mod_path: &ModulePath,
        _wgsl_source_path: &str,
        _source_map: Option<&BasicSourceMap>,
    ) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
}
