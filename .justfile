bash:
  echo "just > bash"

deploy:
  cargo geng build --platform web --release
  butler push target/geng kuviman/spriteshape:html5
