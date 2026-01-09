import package::test::one;

const min: u32 = 0;

@compute @workgroup_size(5, 6, one)
fn main(
    @builtin(global_invocation_id) absolute_idx: vec3<u32>,
) {
    let two = one + one;

    return;
}
