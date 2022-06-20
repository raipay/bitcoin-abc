#include <chronik_cxx/src/lib.rs.h>

#include <logging.h>

void StartChronik() {
    LogPrintf("Starting Chronik! Integer: %d\n", rusty_cxxbridge_integer());
}
