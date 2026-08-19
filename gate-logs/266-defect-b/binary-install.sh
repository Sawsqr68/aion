cd /Users/tom/Developer/ablative/stack/aion
cargo install --path crates/aion-cli --force -j 6
rc=$?
echo $rc > gate-logs/266-defect-b/binary-install.exit
exit $rc
