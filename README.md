
[![codecov](https://codecov.io/gh/danieleades/tide-upload-limit/branch/main/graph/badge.svg?token=WE7SKGCVH0)](https://codecov.io/gh/danieleades/tide-upload-limit)

# Tide-Upload-Limit

An upload limiting filter [`Middleware`](tide::Middleware) for ['tide']

```rust
use tide_upload_limit::UploadLimit;

// set a global upload limit of 4kb
let upload_limiter = UploadLimit::new(1024 * 4);

let mut app = tide::new();
app.with(upload_limiter);
```

## License

* MIT license (https://opensource.org/licenses/MIT)
