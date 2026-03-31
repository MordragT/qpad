build:
    tailwindcss -o crates/web/static/style.css --content "crates/web/templates/**/*.html"

watch:
    tailwindcss -w -o crates/web/static/style.css --content "crates/web/templates/**/*.html"
