// UI module placeholder: rendering helpers will be extracted here incrementally.
pub fn noop() {}
EOF
echo "use ui module"
# register in lib.rs
sed -i 's/^pub mod worker;/pub mod ui;\npub mod worker;/' /run/media/sun/halo/bareBoot/bareflash/crates/bareboot-tui/src/lib.rs
grep -n "pub mod" /run/media/sun/halo/bareBoot/bareflash/crates/bareboot-tui/src/lib.rs