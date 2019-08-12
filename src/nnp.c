#define PY_SSIZE_T_CLEAN
#include <Python.h>

int main(int argc, char *argv[]) {
}

void call_python() {
  PyObject *pName, *pModule, *pFunc;
  PyObject *pArgs, *pValue;
  int i;

  Py_Initialize();

  pName = PyUnicode_decodeFSDefault("lstm");
  pModule = PyImport_Import(pName);
  Py_DECREF(pName);

  if (pModule) {
    pFunc = PyObject_GetAttrString(pModule, "predict");

    if (pFunc && PyCallable_Check(pFunc)) {
      pArgs = PyTuple_New();
    }
  }
}
