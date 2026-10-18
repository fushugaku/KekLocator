// Resolve function tables that the initial automatic analysis did not follow.
// @category Erika
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.math.BigInteger;
import java.util.*;

public class SeedCallbacks extends GhidraScript {
    public void run() throws Exception {
        Set<Long> entries = new TreeSet<>();
        for (int i=0;i<20;i++) entries.add(Integer.toUnsignedLong(getInt(toAddr(0x20000088L+i*72L+64L))));
        for (int i=0;i<36;i++) entries.add(Integer.toUnsignedLong(getInt(toAddr(0x20001234L+i*32L+28L))));
        // The rodata/data load images contain vtables, callback tables and init arrays.
        for (long p=0x60042a00L;p<0x6005d000L;p+=4) {
            long raw=Integer.toUnsignedLong(getInt(toAddr(p)));
            if ((raw&1)!=0 && raw>=0x60020401L && raw<0x60042a00L) entries.add(raw);
        }
        int added=0;
        for (long raw:entries) {
            if ((raw&1)==0 || raw<0x60020401L || raw>=0x60042a00L) continue;
            Address a=toAddr(raw&~1L);
            if (getInstructionAt(a)==null) {
                currentProgram.getProgramContext().setValue(currentProgram.getRegister("TMode"),a,a.add(1),BigInteger.ONE);
                disassemble(a);
            }
            if (getFunctionAt(a)==null && getInstructionAt(a)!=null) {
                Function f=createFunction(a,"Callback_"+Long.toHexString(raw&~1L));
                if (f!=null) added++;
            }
        }
        println("Seeded "+entries.size()+" table pointers; created "+added+" additional functions.");
    }
}
