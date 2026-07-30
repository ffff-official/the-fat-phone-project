cargo build --release
target=`cargo metadata --no-deps --format-version=1 | jq -r '.target_directory'`
bin="$target/release/$1"
sudo setcap cap_net_admin+ep "$bin"
echo $bin
