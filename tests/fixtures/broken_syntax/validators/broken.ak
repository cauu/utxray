// This file has intentional syntax errors
validator broken {
  spend(datum redeemer: Data, _tx: Data) {
    // Missing comma and type annotation - should cause compile error
    True
  }
