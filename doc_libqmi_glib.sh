cargo +nightly rustdoc \
    -p libqmi-glib-rs\
    --all-features \
    --lib \
    -- \
    --cfg docsrs
xdg-open target/doc/libqmi_glib_rs/index.html