# This script takes care of testing your crate

set -ex

cross build --target $TARGET
cross build --target $TARGET --release

if [ ! -z $DISABLE_TESTS ]; then
    return
fi

cross test --target $TARGET
cross test --target $TARGET --release

# quick test, skip on deploy
if [ -z $TRAVIS_TAG ]
then
    #RUST_BACKTRACE=1 cross run --target $TARGET -- -y
    RUST_BACKTRACE=1 target/$TARGET/release/elan-init -- -y
    source ~/.elan/env
    elan which leanpkg
    leanpkg new foo
fi
