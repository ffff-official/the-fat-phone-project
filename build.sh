RUSTFLAGS="-Zsanitizer=address -Cforce-frame-pointers=yes" cargo build -Zbuild-std --target x86_64-unknown-linux-gnu
target=`cargo metadata --no-deps --format-version=1 | jq -r '.target_directory'`
bin="$target/x86_64-unknown-linux-gnu/debug/$1"
sudo setcap cap_net_admin+ep "$bin"
echo $bin
