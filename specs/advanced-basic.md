# Advanced BASIC - Modern Extensions Plan

## Reference Documentation

**BBC BASIC for Windows Manual:** https://www.bbcbasic.co.uk/bbcwin/manual/index.html

This comprehensive manual covers BBC BASIC syntax, functions, statements, and extensions. All new features in this plan maintain backward compatibility with standard BBC BASIC as documented in this manual.

---

## Executive Summary

Extend the Rust-based BBC BASIC interpreter with modern programming paradigms suitable for machine learning and reusable code libraries, while maintaining **full backward compatibility** with original BBC BASIC.

**User Requirements Confirmed:**
- ✅ Standard .bas files for modules (text files with DEF PROC/DEF FN)
- ✅ MAT prefix syntax for matrix operations (e.g., `result = A.MULTIPLY(B)`)
- ✅ Full ML toolkit (linear algebra + statistics + neural network primitives)
- ✅ Full BBC BASIC compatibility (all existing programs must run unchanged)

---

## Implementation Plan

### Phase 1: JSON/Data Serialization (Week 1) - FOUNDATION

**Why First?** Self-contained, simple, useful for ML data loading.

#### Syntax
```basic
REM Load JSON from file
data$ = LOADJSON$("data.json")

REM Parse JSON string into variables
PARSEJSON json$, result$

REM Convert variable to JSON string
TOJSON variable$, output$

REM Save variable to JSON file
SAVEJSON "output.json", variable$
```

#### New Module: `src/json/mod.rs`
```rust
pub struct JsonOps;

impl JsonOps {
    pub fn parse_json(json_str: &str, target: &mut VariableStore, var_name: &str) -> Result<()>;
    pub fn to_json(vars: &VariableStore, var_name: &str) -> Result<String>;
    pub fn load_json_file(path: &str) -> Result<String>;
    pub fn save_json_file(vars: &VariableStore, var_name: &str, path: &str) -> Result<()>;
}
```

#### Files to Modify
- `src/json/mod.rs` - NEW FILE - JSON operations
- `src/parser/mod.rs` - Add Statement::ParseJson, Statement::ToJson, Statement::SaveJson
- `src/executor/mod.rs` - Add execute_parse_json(), execute_to_json(), execute_save_json()
- `src/tokenizer/mod.rs` - Add LOADJSON$, PARSEJSON, TOJSON, SAVEJSON keywords
- `Cargo.toml` - Add `serde_json = "1.0"` and `serde = { version = "1.0", features = ["derive"] }`

#### Testing: 20+ unit tests

---

### Phase 2: Advanced File Operations (Week 2)

**Why Second?** Extends existing file I/O, straightforward.

#### Syntax
```basic
REM Random access
SEEK# channel, position
result = TELL#channel

REM Directory operations
files$ = LISTDIR$("path")
MKDIR "dirname"
RMDIR "dirname"

REM File metadata
size = FILESIZE("file.dat")
exists = FILEEXISTS("data.txt")

REM Binary I/O
BPUT# channel, byte_value
byte = BGET#channel
```

#### Files to Modify
- `src/filesystem/mod.rs` - Expand from 23 lines to full implementation
- `src/parser/mod.rs` - Add Statement::SeekFile, Statement::TellFile, Statement::ListDir, Statement::MakeDir, Statement::RemoveDir, Statement::FileSize, Statement::FileExists, Statement::BPutFile, Statement::BGetFile
- `src/executor/mod.rs` - Add execute_seek(), execute_tell(), execute_listdir(), execute_mkdir(), execute_rmdir(), execute_filesize(), execute_fileexists(), execute_bput(), execute_bget()
- `src/tokenizer/mod.rs` - Add SEEK, TELL, LISTDIR$, MKDIR, RMDIR, FILESIZE, FILEEXISTS, BPUT, BGET keywords

#### Testing: 30+ unit tests

---

### Phase 3: Module/Library System (Week 3-4)

**Why Third?** Moderate complexity, builds on file operations.

#### Syntax
```basic
REM Import a standard .bas file as a module
IMPORT "mathlib.bas"
IMPORT "utils/stats.bas" AS stats

REM Call imported procedures (namespace optional)
PROC mathlib.add(10, 20)
PROC stats.mean(data())
```

#### New Module: `src/modules/mod.rs`
```rust
pub struct ModuleRegistry {
    modules: HashMap<String, (PathBuf, ProgramStore)>,
    namespaces: HashMap<String, String>,
}

impl ModuleRegistry {
    pub fn import(&mut self, filename: &str, alias: Option<&str>) -> Result<()>;
    pub fn get_procedure(&self, module: &str, name: &str) -> Option<&ProcedureDefinition>;
    pub fn get_function(&self, module: &str, name: &str) -> Option<&FunctionDefinition>;
}
```

#### Files to Modify
- `src/modules/mod.rs` - NEW FILE - Module registry and management
- `src/parser/mod.rs` - Add Statement::Import with filename and alias
- `src/executor/mod.rs` - Add ModuleRegistry field, execute_import(), update procedure/function call to search modules
- `src/tokenizer/mod.rs` - Add IMPORT keyword (0xC8, 0xB0 extended token)
- `src/program/mod.rs` - May need to expose ProgramStore for module access

#### Sample Modules to Create
- `examples/modules/mathlib.bas` - Math procedures
- `examples/modules/utils/stats.bas` - Statistical functions

#### Testing: 25+ unit tests

---

### Phase 4: Matrix Math Operations - Full ML Toolkit (Week 5-6)

**Why Last?** Most complex, requires new expression syntax.

#### Syntax (MAT prefix - object-oriented style)
```basic
REM Matrix creation
DIM A(3, 3), B(3, 3), C(3, 3)

REM Linear algebra
result = A.MULTIPLY(B)        REM Matrix multiplication
result = A.ADD(B)             REM Element-wise addition
result = A.SUBTRACT(B)        REM Element-wise subtraction
result = A.SCALAR(2.5)        REM Scalar multiplication

REM Transformations
result = A.TRANSPOSE()        REM Transpose
result = A.INVERSE()          REM Matrix inverse
det = A.DETERMINANT()         REM Determinant

REM Statistics
mean = A.MEAN()               REM Mean of all elements
std = A.STDEV()              REM Standard deviation
sum = A.SUM()                REM Sum
min = A.MIN()                REM Minimum
max = A.MAX()                REM Maximum

REM Neural network primitives
result = A.RELU()             REM ReLU activation
result = A.SIGMOID()          REM Sigmoid activation
result = A.TANH()             REM Tanh activation
result = A.SOFTMAX()          REM Softmax
result = A.DOTPRODUCT(B)      REM Dot product
```

#### New Module: `src/matrix/mod.rs`
```rust
pub struct MatrixOps;

impl MatrixOps {
    // Linear algebra
    pub fn multiply(vars: &VariableStore, a_name: &str, b_name: &str) -> Result<Vec<f64>>;
    pub fn add(vars: &VariableStore, a_name: &str, b_name: &str) -> Result<Vec<f64>>;
    pub fn subtract(vars: &VariableStore, a_name: &str, b_name: &str) -> Result<Vec<f64>>;
    pub fn transpose(vars: &VariableStore, a_name: &str) -> Result<(Vec<f64>, Vec<usize>)>;
    pub fn inverse(vars: &VariableStore, a_name: &str) -> Result<Vec<f64>>;
    pub fn determinant(vars: &VariableStore, a_name: &str) -> Result<f64>;

    // Statistics
    pub fn mean(vars: &VariableStore, a_name: &str) -> Result<f64>;
    pub fn stdev(vars: &VariableStore, a_name: &str) -> Result<f64>;
    pub fn sum(vars: &VariableStore, a_name: &str) -> Result<f64>;
    pub fn min(vars: &VariableStore, a_name: &str) -> Result<f64>;
    pub fn max(vars: &VariableStore, a_name: &str) -> Result<f64>;

    // Neural network primitives
    pub fn relu(vars: &VariableStore, a_name: &str) -> Result<Vec<f64>>;
    pub fn sigmoid(vars: &VariableStore, a_name: &str) -> Result<Vec<f64>>;
    pub fn tanh(vars: &VariableStore, a_name: &str) -> Result<Vec<f64>>;
    pub fn softmax(vars: &VariableStore, a_name: &str) -> Result<Vec<f64>>;
    pub fn dotproduct(vars: &VariableStore, a_name: &str, b_name: &str) -> Result<f64>;
}
```

#### Parser Changes (Critical)
Add new Expression variant:
```rust
pub enum Expression {
    MatrixMethod {
        matrix: String,           // Matrix variable name
        method: MatrixMethod,     // Method to call
        args: Vec<Expression>,    // Arguments (for multiply, scalar, etc.)
    },
}

pub enum MatrixMethod {
    Multiply, Add, Subtract, Scalar, Transpose, Inverse, Determinant,
    ReLU, Sigmoid, Tanh, Softmax, DotProduct,
    Mean, StDev, Sum, Min, Max,
}
```

#### Files to Modify
- `src/matrix/mod.rs` - NEW FILE - Matrix operations using nalgebra
- `src/parser/mod.rs` - Add Expression::MatrixMethod, parse_matrix_method_call()
- `src/executor/mod.rs` - Add eval_matrix_method(), execute_matrix_assignment()
- `src/tokenizer/mod.rs` - Add MAT keywords (0xD0-0xE0 range)
- `Cargo.toml` - Add `nalgebra = "0.32"`

#### Testing: 40+ unit tests including ML examples

---

## Critical Implementation Files

| File | Purpose | Changes |
|------|---------|---------|
| `src/parser/mod.rs` | Parse new syntax | Add Expression::MatrixMethod, Statement::Import, JSON/file ops |
| `src/executor/mod.rs` | Execute new operations | Add module registry, matrix eval, JSON handlers, file ops |
| `src/tokenizer/mod.rs` | Define new keywords | Add IMPORT, MAT* methods, JSON functions, file ops |
| `src/variables/mod.rs` | Variable storage | May need matrix metadata extensions |
| `Cargo.toml` | Dependencies | Add serde_json, serde, nalgebra |

---

## Dependencies to Add

```toml
[dependencies]
# Existing
rand = "0.8"

# New additions
serde_json = "1.0"     # JSON parsing/generation (~45 KB)
serde = { version = "1.0", features = ["derive"] }  # (~70 KB)
nalgebra = "0.32"      # Linear algebra for ML (~500 KB)
```

**Decision**: Use nalgebra for matrix operations - performance and correctness benefits outweigh binary size for ML applications.

---

## Backward Compatibility Strategy

1. **Extended Token Range**: All new keywords use 0xC8 prefix (0xB0-0xE0 available)
2. **No Syntax Conflicts**: New patterns (IMPORT, MAT prefix, JSON functions) don't overlap existing BASIC
3. **Graceful Errors**: Unknown tokens produce proper errors, not crashes
4. **Test Suite**: Run all 175 existing tests after each phase to verify no regressions

---

## Testing Strategy

### Unit Tests per Module
- JSON: 20+ tests
- File operations: 30+ tests
- Modules: 25+ tests
- Matrix: 40+ tests
- **Total: 115+ new tests**

### Integration Tests
- ML pipeline: Load JSON → Create matrices → Process → Save results
- Module usage: Import matrix library → Use in ML workflow
- File operations: Random access + binary I/O

### Compatibility Tests
- All existing .bas programs must run unchanged
- All 175 existing tests must pass

---

## Example Programs to Create

```
examples/
├── modules/
│   ├── mathlib.bas          - Math functions library
│   └── utils/stats.bas      - Statistical procedures
├── ml/
│   ├── neural_net.bas       - Simple neural network example
│   ├── linear_reg.bas       - Linear regression
│   └── data_loader.bas      - Load JSON data for ML
├── file_ops/
│   ├── random_access.bas    - SEEK#/TELL# demo
│   └── directory_ops.bas    - MKDIR/RMDIR demo
└── json/
    ├── parse_json.bas       - Parse JSON example
    └── to_json.bas          - Generate JSON example
```

---

### Phase 5: Struct/Record Types (Week 7)

**Why This?** ML needs structured data, not just flat variables.

#### Syntax
```basic
REM Define a record type
TYPE DataPoint
    features(10)
    label%
    name$
ENDTYPE

REM Create instances
DIM point AS DataPoint
point.features(0) = 1.5
point.label% = 1

REM Arrays of records (essential for ML datasets)
DIM training(1000) AS DataPoint
FOR i = 0 TO 999
    training(i).label% = i MOD 2
NEXT i

REM Nested structs
TYPE NeuralLayer
    weights(20, 20)
    biases(20)
    activation$
ENDTYPE

TYPE Network
    layers(5) AS NeuralLayer
    learning_rate
ENDTYPE

DIM net AS Network
```

#### Files to Modify
- `src/types/mod.rs` - NEW FILE - Type definitions and struct registry
- `src/variables/mod.rs` - Add Variable::Struct variant, field access
- `src/parser/mod.rs` - Add Statement::TypeDefinition, parse field access with `.` operator
- `src/executor/mod.rs` - Execute type definitions, handle struct assignment

#### Compatibility: ✅ ZERO RISK
- New keywords `TYPE`/`ENDTYPE` don't conflict
- `AS typename` in DIM is new syntax
- Dot notation `struct.field` is new (no conflict with existing syntax)

---

### Phase 6: List/Map Collections (Week 8)

**Why This?** Dynamic data structures essential for modern programming.

#### Syntax
```basic
REM Lists (dynamic arrays that grow)
list = NEWLIST()
LISTADD list, 42
LISTADD list, "hello"
LISTADD list, 3.14

count = LISTLEN(list)      REM Get length
item = LISTGET(list, 0)    REM Get by index
LISTREMOVE list, 0         REM Remove by index
LISTCLEAR list             REM Clear all

REM Iterate over list
FOR EACH item IN list
    PRINT item
NEXT EACH

REM Maps/dictionaries (hash tables)
dict = NEWMAP()
MAPPUT dict, "name", "Alice"
MAPPUT dict, "age", 30
MAPPUT dict, "active", TRUE

value$ = MAPGET$(dict, "name")     REM "Alice"
IF MAPHAS(dict, "email") THEN ...
MAPREMOVE dict, "age"
keys$() = MAPKEYS$(dict)           REM Get all keys as array

REM Nested structures (useful for JSON-like data)
config = NEWMAP()
MAPPUT config, "layers", NEWLIST()
LISTADD MAPGET$(config, "layers"), NEWMAP()
```

#### Files to Modify
- `src/collections/mod.rs` - NEW FILE - List and Map implementations
- `src/parser/mod.rs` - Add Expression::NewList, Expression::NewMap, collection operations
- `src/executor/mod.rs` - Implement list/map operations
- `src/variables/mod.rs` - Add Variable::List, Variable::Map variants

#### Dependencies
```toml
indexmap = "2.0"  # For ordered maps (preserves insertion order)
```

#### Compatibility: ✅ ZERO RISK
- All new function names (NEWLIST, NEWMAP, etc.)
- FOR EACH syntax is new but doesn't conflict with FOR...NEXT
- No changes to existing behavior

---

### Phase 7: Pattern Matching (Week 9)

**Why This?** Cleaner than nested IF for ML pipelines and command processing.

#### Syntax
```basic
REM Match on string value
MATCH command$
    CASE "train"
        PROC train_model()
    CASE "predict"
        PROC predict(input)
    CASE "export"
        PROC export_model()
    CASE OTHER
        PRINT "Unknown command: "; command$
ENDMATCH

REM Match on numeric value
MATCH score%
    CASE < 60
        grade$ = "F"
    CASE 60 TO 69
        grade$ = "D"
    CASE 70 TO 79
        grade$ = "C"
    CASE 80 TO 89
        grade$ = "B"
    CASE >= 90
        grade$ = "A"
ENDMATCH

REM Match on struct type
MATCH data
    CASE AS Image
        PROC process_image(data)
    CASE AS Audio
        PROC process_audio(data)
    CASE AS Text
        PROC process_text(data)
ENDMATCH
```

#### Files to Modify
- `src/parser/mod.rs` - Add Statement::Match with Case arms
- `src/executor/mod.rs` - Execute match with pattern evaluation
- `src/tokenizer/mod.rs` - Add MATCH, CASE, OTHER, ENDMATCH keywords

#### Compatibility: ✅ ZERO RISK
- Entirely new control structure
- No overlap with existing IF/ELSE/ON...GOTO
- CASE keyword exists in some BASIC dialects but not BBC BASIC

---

### Phase 8: Shell Command Execution (Week 10)

**Why This?** Enable system integration and scripting capabilities.

#### Syntax
```basic
REM Execute shell command and get output
output$ = SHELL("ls -al")
PRINT output$

REM With error handling
output$ = SHELL("gcc --version")
IF SHELLSTATUS%() <> 0 THEN
    PRINT "Command failed: "; SHELLERROR$()
ENDIF

REM Execute without capturing output (fire and forget)
SHELL "mkdir -p ./data/output"

REM Chained commands (pipelines)
results$ = SHELL("cat data.csv | grep 'pattern' | wc -l")

REM Interactive commands
SHELL "vim config.txt"
```

#### Implementation
```rust
// New module: src/shell/mod.rs
pub struct ShellOps;

impl ShellOps {
    /// Execute shell command and capture output
    pub fn execute(command: &str) -> Result<String>;

    /// Execute without capturing output
    pub fn execute_interactive(command: &str) -> Result<()>;

    /// Get exit status of last command
    pub fn get_status() -> i32;

    /// Get error message from last failed command
    pub fn get_error() -> String;
}
```

#### Files to Modify
- `src/shell/mod.rs` - NEW FILE - Shell command execution using std::process
- `src/parser/mod.rs` - Add Expression::Shell (function-style), Statement::ShellExecute
- `src/executor/mod.rs` - Add shell execution methods
- `src/tokenizer/mod.rs` - Add SHELL keyword, SHELLSTATUS%, SHELLERROR$ functions

#### Platform Considerations
```rust
#[cfg(unix)]
use std::process::Command;

#[cfg(windows)]
// Handle Windows-specific command syntax (cmd vs bash)
```

#### Security Considerations
- **WARNING**: Shell injection risk if user input is passed directly
```basic
REM DANGEROUS - don't do this:
filename$ = INPUT$("Enter file: ")
output$ = SHELL("cat " + filename$)  REM Could be "; rm -rf /"

REM SAFER - sanitize or use whitelist:
IF VALIDNAME$(filename$) THEN
    output$ = SHELL("cat ./safe/" + filename$)
ENDIF
```

#### Compatibility: ✅ ZERO RISK
- SHELL is a new function/statement
- No conflict with existing BBC BASIC (OSCLI exists but different syntax)

---

## Compatibility Analysis Summary

| Phase | Feature | Compatibility Risk | Breaking Changes |
|-------|---------|-------------------|------------------|
| 1 | JSON/Data Serialization | ✅ Zero | None |
| 2 | Advanced File Operations | ✅ Zero | None |
| 3 | Module/Library System | ✅ Zero | None |
| 4 | Matrix Math Operations | ✅ Zero | None |
| 5 | Struct/Record Types | ✅ Zero | None |
| 6 | List/Map Collections | ✅ Zero | None |
| 7 | Pattern Matching | ✅ Zero | None |
| 8 | Shell Command Execution | ✅ Zero | None |

**All proposed features maintain full BBC BASIC compatibility.**

### Features Considered But Rejected

| Feature | Reason for Rejection |
|---------|---------------------|
| Type annotations with `:` syntax | Conflicts with statement separator (`:`) |
| Async/Await | Changes execution model, adds complexity |
| First-class functions with closures | DEF FN sufficient for now |
| Regular expressions | Can be added via external modules |
| HTTP operations | Can use SHELL with curl/wget |

---

## Extended Dependencies

```toml
[dependencies]
# Existing
rand = "0.8"

# From Phases 1-4
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
nalgebra = "0.32"

# New for Phases 5-8
indexmap = "2.0"  # For ordered maps in collections

# Shell execution uses std::process (no external dependency needed)
```

---

## Updated Testing Strategy

### Unit Tests per Module
- JSON: 20+ tests
- File operations: 30+ tests
- Modules: 25+ tests
- Matrix: 40+ tests
- Structs: 30+ tests
- Collections: 35+ tests
- Pattern matching: 20+ tests
- Shell: 15+ tests
- **Total: 215+ new tests**

### Integration Tests
- ML pipeline with structs: Load JSON → Create structured dataset → Train model
- Collection operations: Build dataset using lists/maps → Export to JSON
- Shell integration: Run external ML tools → Parse output → Process results
- Pattern matching: Command processor for ML workflow

### Compatibility Tests
- All existing .bas programs run unchanged
- All 175 existing tests pass
- Test suite runs on Linux, macOS, Windows

---

## Updated Example Programs

```
examples/
├── modules/
│   ├── mathlib.bas          - Math functions library
│   └── utils/stats.bas      - Statistical procedures
├── ml/
│   ├── neural_net.bas       - Simple neural network
│   ├── linear_reg.bas       - Linear regression
│   ├── data_loader.bas      - Load JSON data
│   └── structured_data.bas  - Using structs for datasets
├── file_ops/
│   ├── random_access.bas    - SEEK#/TELL# demo
│   └── directory_ops.bas    - MKDIR/RMDIR demo
├── json/
│   ├── parse_json.bas       - Parse JSON
│   └── to_json.bas          - Generate JSON
├── collections/
│   ├── list_demo.bas        - List operations
│   └── map_demo.bas         - Map/dictionary operations
├── shell/
│   ├── system_commands.bas  - Shell execution
│   └── pipeline.bas         - Chained commands
└── patterns/
    └── command_processor.bas - Pattern matching demo
```

---

### Phase 9: Python Library Integration (Week 11) - OPTIONAL

**Why This?** Instant access to entire Python ML ecosystem (numpy, pandas, scikit-learn, tensorflow, pytorch).

**Approach:** Embed CPython interpreter using `pyo3` crate. Feature-flagged compilation keeps default binary small.

#### Syntax
```basic
REM Import Python modules with optional alias
IMPORT PYTHON "numpy" AS np
IMPORT PYTHON "pandas" AS pd
IMPORT PYTHON "sklearn.linear_model" AS sklearn
IMPORT PYTHON "torch"  REM PyTorch!

REM Create Python objects (automatic type conversion)
data = np.array([1, 2, 3, 4, 5])
matrix = np.zeros((3, 3))
df = pd.read_csv("data.csv")

REM Call Python functions
mean = np.mean(data)
result = np.dot(matrix_a, matrix_b)

REM Method chaining on Python objects
sorted = df.sort_values("column")
filtered = df[df["age"] > 25]

REM Full ML pipeline
model = sklearn.LinearRegression()
model.fit(X_train, y_train)
predictions = model.predict(X_test)
score = model.score(X_test, y_test)
PRINT "Accuracy: "; score

REM Deep learning with PyTorch
import torch
net = torch.nn.Linear(10, 5)
output = net.forward(input_tensor)
```

#### Implementation

**New Module: `src/python/mod.rs`**
```rust
use pyo3::{Python, PyResult, PyModule, types::PyAny};
use std::collections::HashMap;

pub struct PythonBridge {
    modules: HashMap<String, Py<PyModule>>,
    last_result: Option<PyObject>,
}

impl PythonBridge {
    pub fn new() -> Self {
        Python::with_gil(|py| {
            // Initialize Python interpreter
            pyo3::prepare_freethreaded_python();
        });
        Self {
            modules: HashMap::new(),
            last_result: None,
        }
    }

    /// Import Python module
    pub fn import_module(&mut self, name: &str, alias: &str) -> Result<()> {
        Python::with_gil(|py| {
            let module = py.import(name)
                .map_err(|e| BBCBasicError::PythonError(e.to_string()))?;
            self.modules.insert(alias.to_string(), module.into());
            Ok(())
        })
    }

    /// Call Python function: module.function(args...)
    pub fn call_function(&mut self, module: &str, func: &str, args: &[Value]) -> Result<Value> {
        Python::with_gil(|py| {
            let module_ref = self.modules.get(module)
                .ok_or(BBCBasicError::ModuleNotFound(module.to_string()))?;
            let module_py: &PyModule = module_ref.as_ref(py);

            let func_obj = module_py.getattr(func)
                .map_err(|e| BBCBasicError::PythonError(e.to_string()))?;

            let py_args = convert_basic_to_python(args, py)?;
            let result = func_obj.call(py_args, None)
                .map_err(|e| BBCBasicError::PythonError(e.to_string()))?;

            self.last_result = Some(result.into());
            convert_python_to_basic(result)
        })
    }

    /// Get attribute from Python object: module.attr
    pub fn get_attribute(&mut self, module: &str, attr: &str) -> Result<Value> {
        Python::with_gil(|py| {
            let module_ref = self.modules.get(module)
                .ok_or(BBCBasicError::ModuleNotFound(module.to_string()))?;
            let module_py: &PyModule = module_ref.as_ref(py);

            let attr_obj = module_py.getattr(attr)
                .map_err(|e| BBCBasicError::PythonError(e.to_string()))?;

            convert_python_to_basic(attr_obj)
        })
    }
}

/// Convert BASIC values to Python objects
fn convert_basic_to_python(values: &[Value], py: Python) -> Result<PyObject> {
    let py_values: Result<Vec<PyObject>> = values.iter()
        .map(|v| match v {
            Value::Integer(n) => Ok(n.to_object(py)),
            Value::Real(f) => Ok(f.to_object(py)),
            Value::String(s) => Ok(s.to_object(py)),
            Value::IntegerArray(arr) => {
                // Convert to numpy array
                let numpy = py.import("numpy")?;
                numpy.call_method("array", (arr.values.clone(),), None)
                    .map_err(|e| BBCBasicError::PythonError(e.to_string()))
            }
            _ => Err(BBCBasicError::TypeMismatch),
        })
        .collect();

    Ok(py_values?.to_object(py))
}

/// Convert Python objects to BASIC values
fn convert_python_to_basic(obj: &PyAny) -> Result<Value> {
    use pyo3::types::PyFloat;
    use pyo3::types::PyList;
    use pyo3::types::PyString;

    if let Ok(s) = obj.downcast::<PyString>() {
        Ok(Value::String(s.to_string_lossy().to_string()))
    } else if let Ok(f) = obj.downcast::<PyFloat>() {
        Ok(Value::Real(f.value()))
    } else if let Ok(list) = obj.downcast::<PyList>() {
        // Convert Python list to BASIC array
        let mut values = Vec::new();
        for item in list.iter() {
            values.push(convert_python_to_basic(item)?);
        }
        Ok(Value::List(values))
    } else {
        // For unsupported types, convert to string representation
        Ok(Value::String(obj.repr()?.to_string()))
    }
}
```

#### Files to Modify
- `src/python/mod.rs` - NEW FILE - Python bridge using pyo3
- `src/parser/mod.rs` - Add Statement::ImportPython with module and alias
- `src/executor/mod.rs` - Add PythonBridge field, execute_python_import(), handle Python calls
- `src/tokenizer/mod.rs` - Add PYTHON keyword for IMPORT PYTHON syntax
- `Cargo.toml` - Add feature-flagged pyo3 dependency

#### Cargo.toml Configuration
```toml
[dependencies]
# Existing dependencies...

[features]
default = []
python-integration = ["pyo3"]

[dependencies.pyo3]
version = "0.20"
optional = true
features = ["auto-initialize"]
```

#### Build Commands
```bash
# Default build (small binary, ~2MB)
cargo build --release

# With Python support (larger binary, ~50MB)
cargo build --release --features python-integration

# Check if Python support is enabled
#[cfg(feature = "python-integration")]
compile_error!("Python integration enabled");

#[cfg(not(feature = "python-integration"))]
compile_error!("Python integration disabled");
```

#### Runtime Detection
```rust
// At runtime, check if Python support is available
#[cfg(feature = "python-integration")]
{
    // Python bridge available
    executor.import_python("numpy", "np")?;
}

#[cfg(not(feature = "python-integration"))]
{
    // Graceful error
    return Err(BBCBasicError::FeatureNotAvailable("Python integration".to_string()));
}
```

#### Error Handling
```basic
REM Python errors are caught and reported
TRY
    data = np.array([1, 2, 3])
CATCH err
    PRINT "Python error: "; err.message$
ENDTRY
```

#### Binary Size Impact

| Build Configuration | Binary Size | Python Support |
|---------------------|-------------|----------------|
| Default (no features) | ~2 MB | None |
| --features python-integration | ~50 MB | Embedded CPython |
| --features python-integration + stdlib | ~100 MB+ | Full Python |

**Recommendation:** Ship two binaries:
- `bbc-basic` (~2 MB) - Default build for general use
- `bbc-basic-ml` (~50 MB) - With Python for ML users

#### Type Conversion Rules

| BASIC Type | Python Type | Notes |
|------------|-------------|-------|
| Integer (42%) | int | Direct conversion |
| Real (3.14) | float | Direct conversion |
| String ("hello") | str | Direct conversion |
| IntegerArray | numpy.ndarray | Automatic conversion |
| RealArray | numpy.ndarray | Automatic conversion |
| List | list | Recursive conversion |
| Map | dict | Recursive conversion |

| Python Type | BASIC Type | Notes |
|-------------|------------|-------|
| int | Integer | Direct conversion |
| float | Real | Direct conversion |
| str | String | Direct conversion |
| list | List | Recursive conversion |
| dict | Map | Recursive conversion |
| numpy.ndarray | RealArray | Preserves dimensions |
| Other | String | Repr() fallback |

#### Platform Considerations
```rust
#[cfg(all(feature = "python-integration", unix))]
fn find_python() -> PathBuf {
    // Use system Python or embedded
    if let Ok(py) = std::env::var("PYTHON_PATH") {
        PathBuf::from(py)
    } else {
        PathBuf::from("python3")
    }
}

#[cfg(all(feature = "python-integration", windows))]
fn find_python() -> PathBuf {
    // Windows Python detection
    PathBuf::from("python.exe")
}
```

#### Security Considerations

**WARNING:** Python code execution has security implications.

```basic
REM DANGEROUS - arbitrary code execution
code$ = INPUT$("Enter Python code: ")
result = EVAL(code$)  REM DON'T DO THIS

REM SAFER - only allow whitelisted modules
IMPORT PYTHON "numpy" AS np  REM Safe
IMPORT PYTHON "os" AS os      REM Potentially dangerous

REM Recommendation: Blacklist dangerous modules
REM os, subprocess, shutil, sys, eval, exec, etc.
```

#### Recommended Module Whitelist

**Safe for ML:**
- numpy, scipy, pandas
- sklearn, tensorflow, pytorch
- matplotlib, seaborn
- PIL, opencv

**Potentially Dangerous (optional blacklist):**
- os, subprocess, shutil (file system access)
- sys, importlib (runtime modification)
- eval, exec, compile (code execution)
- socket, http (network access)

#### Compatibility: ✅ ZERO RISK
- New `IMPORT PYTHON` syntax doesn't conflict
- Feature-flagged - default build unaffected
- Runtime errors if Python code fails (doesn't crash interpreter)

#### Dependencies
```toml
[dependencies]
pyo3 = { version = "0.20", optional = true, features = ["auto-initialize"] }
```

#### Testing: 20+ tests
- Import common modules (numpy, pandas)
- Type conversion (BASIC ↔ Python)
- Error handling (Python exceptions → BASIC errors)
- Method chaining
- ML workflow integration test

---

## Compatibility Analysis Summary (Updated)

| Phase | Feature | Compatibility Risk | Binary Size Impact |
|-------|---------|-------------------|-------------------|
| 1 | JSON/Data Serialization | ✅ Zero | +115 KB |
| 2 | Advanced File Operations | ✅ Zero | 0 KB |
| 3 | Module/Library System | ✅ Zero | 0 KB |
| 4 | Matrix Math Operations | ✅ Zero | +500 KB |
| 5 | Struct/Record Types | ✅ Zero | 0 KB |
| 6 | List/Map Collections | ✅ Zero | +50 KB |
| 7 | Pattern Matching | ✅ Zero | 0 KB |
| 8 | Shell Command Execution | ✅ Zero | 0 KB |
| 9 | Python Integration (optional) | ✅ Zero | +48 MB (optional) |

**Default build size: ~3.6 MB**
**With Python: ~52 MB**

---

## Extended Dependencies (Updated)

```toml
[dependencies]
# Existing
rand = "0.8"

# From Phases 1-4
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
nalgebra = "0.32"

# From Phases 5-8
indexmap = "2.0"

# Phase 9 (optional)
pyo3 = { version = "0.20", optional = true, features = ["auto-initialize"] }

[features]
default = []
python-integration = ["pyo3"]
```

---

## Updated Testing Strategy

### Unit Tests per Module (Updated)
- JSON: 20+ tests
- File operations: 30+ tests
- Modules: 25+ tests
- Matrix: 40+ tests
- Structs: 30+ tests
- Collections: 35+ tests
- Pattern matching: 20+ tests
- Shell: 15+ tests
- Python integration: 20+ tests
- **Total: 235+ new tests**

### Integration Tests (Updated)
- ML pipeline with structs: Load JSON → Create structured dataset → Train model
- Collection operations: Build dataset using lists/maps → Export to JSON
- Shell integration: Run external ML tools → Parse output → Process results
- Pattern matching: Command processor for ML workflow
- **NEW:** Full Python ML workflow: pandas load → sklearn train → evaluate → save results

### Compatibility Tests
- All existing .bas programs run unchanged
- All 175 existing tests pass
- Test suite runs on Linux, macOS, Windows
- **NEW:** Verify default build without Python feature still works

---

## Updated Example Programs

```
examples/
├── modules/
│   ├── mathlib.bas          - Math functions library
│   └── utils/stats.bas      - Statistical procedures
├── ml/
│   ├── neural_net.bas       - Simple neural network (BASIC matrices)
│   ├── linear_reg.bas       - Linear regression (BASIC matrices)
│   ├── data_loader.bas      - Load JSON data
│   ├── structured_data.bas  - Using structs for datasets
│   ├── sklearn_demo.bas     - scikit-learn via Python (NEW!)
│   └── pytorch_demo.bas     - PyTorch neural network (NEW!)
├── file_ops/
│   ├── random_access.bas    - SEEK#/TELL# demo
│   └── directory_ops.bas    - MKDIR/RMDIR demo
├── json/
│   ├── parse_json.bas       - Parse JSON
│   └── to_json.bas          - Generate JSON
├── collections/
│   ├── list_demo.bas        - List operations
│   └── map_demo.bas         - Map/dictionary operations
├── shell/
│   ├── system_commands.bas  - Shell execution
│   └── pipeline.bas         - Chained commands
└── patterns/
    └── command_processor.bas - Pattern matching demo
```

---

## Status: READY FOR IMPLEMENTATION

Plan complete with 9 phases of zero-risk extensions.
All features maintain full BBC BASIC backward compatibility.

### Implementation Priority Summary

**Essential for ML (Phases 1-4):**
- JSON data loading
- Advanced file operations
- Module system for reusable code
- Matrix operations

**Modern Programming Essentials (Phases 5-8):**
- Structured data types
- Dynamic collections
- Pattern matching
- System integration

**Advanced ML Capabilities (Phase 9 - Optional):**
- Python library integration (numpy, pandas, scikit-learn, pytorch)
- Feature-flagged compilation (default: small binary)
- Optional build: --features python-integration

### Build Configurations

```bash
# Standard build (~3.6 MB) - No Python
cargo build --release

# ML build (~52 MB) - With Python support
cargo build --release --features python-integration

# Development build
cargo build
```

### Distribution Strategy

**Option A:** Single binary with feature detection
- User chooses at build time
- Two releases: `bbc-basic` and `bbc-basic-ml`

**Option B:** Runtime plugin loading
- Detect Python at runtime
- Dynamically load Python bridge if available
- More complex, smaller default binary

**Recommendation:** Option A (feature-flagged compilation) for simplicity and reliability.
