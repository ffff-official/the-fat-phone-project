cargo +nightly rustdoc \
    -p qmi-sim-manager \
    --all-features \
    --lib \
    -- \
    --cfg docsrs
xdg-open target/doc/qmi_sim_manager/index.html