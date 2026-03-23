validator hello {
  spend(_datum: Option<Data>, _redeemer: Data, _own_ref: Data, _tx: Data) {
    True
  }
}

test hello_always_passes() {
  hello.spend(None, Void, Void, Void)
}
