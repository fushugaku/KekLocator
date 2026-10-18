// @category Erika
import ghidra.app.script.GhidraScript;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.*;
import ghidra.program.model.pcode.*;
import com.google.gson.GsonBuilder;
import java.nio.file.*;
import java.util.*;

public class ExportPcode extends GhidraScript {
    private Map<String,Object> node(Varnode v) {
        if (v == null) return null;
        Map<String,Object> value = new LinkedHashMap<>();
        value.put("space", v.getAddress().getAddressSpace().getName());
        value.put("offset", v.getOffset()); value.put("size", v.getSize());
        return value;
    }
    public void run() throws Exception {
        Map<String,Object> result = new LinkedHashMap<>();
        result.put("language", currentProgram.getLanguageID().toString());
        List<Object> registers = new ArrayList<>();
        for (Register reg : currentProgram.getLanguage().getRegisters()) {
            Map<String,Object> r = new LinkedHashMap<>();
            r.put("name",reg.getName()); r.put("offset",reg.getAddress().getOffset());
            r.put("size",reg.getMinimumByteSize()); registers.add(r);
        }
        result.put("registers",registers);
        List<Object> functions = new ArrayList<>();
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            Map<String,Object> row = new LinkedHashMap<>();
            row.put("name",f.getName()); row.put("entry",f.getEntryPoint().getOffset());
            functions.add(row);
        }
        result.put("functions",functions);
        List<Object> instructions = new ArrayList<>();
        InstructionIterator iter = currentProgram.getListing().getInstructions(true);
        while (iter.hasNext()) {
            Instruction i = iter.next();
            Map<String,Object> row = new LinkedHashMap<>();
            row.put("address",i.getAddress().getOffset()); row.put("length",i.getLength());
            row.put("assembly",i.toString());
            row.put("fallthrough",i.getFallThrough()==null?null:i.getFallThrough().getOffset());
            List<Long> flows = new ArrayList<>();
            for (ghidra.program.model.address.Address address : i.getFlows()) flows.add(address.getOffset());
            row.put("flows",flows);
            List<Object> operations = new ArrayList<>();
            for (PcodeOp op : i.getPcode()) {
                Map<String,Object> p = new LinkedHashMap<>();
                p.put("op",op.getMnemonic()); p.put("out",node(op.getOutput()));
                List<Object> inputs = new ArrayList<>();
                for (Varnode v : op.getInputs()) inputs.add(node(v));
                p.put("in",inputs);
                if (op.getOpcode()==PcodeOp.CALLOTHER) p.put("userop",currentProgram.getLanguage().getUserDefinedOpName((int)op.getInput(0).getOffset()));
                operations.add(p);
            }
            row.put("pcode",operations); instructions.add(row);
        }
        result.put("instructions",instructions);
        Files.writeString(Paths.get(getScriptArgs()[0]),new GsonBuilder().create().toJson(result));
        println("Exported "+instructions.size()+" instructions and "+functions.size()+" function entries.");
    }
}
