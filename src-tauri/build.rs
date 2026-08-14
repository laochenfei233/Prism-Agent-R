fn main() {
	tauri_build::build();

	// Windows 测试二进制需要 comctl32 v6 manifest：能力检测测试调用 RigAgent::run
	// 会拉入 tauri 事件链（muda 菜单），其 TaskDialogIndirect 等符号仅在 comctl32 v6
	// 存在；无 manifest 时 Windows 加载 v5.82，测试 exe 启动即 STATUS_ENTRYPOINT_NOT_FOUND。
	// 详见 docs/compose/specs/agent-capability-tests.md §Windows 测试 manifest。
	#[cfg(target_os = "windows")]
	{
		let manifest = concat!(env!("CARGO_MANIFEST_DIR"), r"\manifests\comctl32-v6.manifest");
		println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
		println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{manifest}");
	}
}
