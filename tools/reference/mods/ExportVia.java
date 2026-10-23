// CPU-only ViaVersion mapping decoder oracle from ViaFabricPlus's embedded JAR.
import com.viaversion.viaversion.api.data.MappingDataLoader;
import com.viaversion.viaversion.api.protocol.version.ProtocolVersion;
import com.viaversion.nbt.tag.*;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import java.util.jar.*;
public class ExportVia {
 public static void main(String[] args)throws Exception{
  var out=new JsonObject();var versions=new JsonArray();for(var version:ProtocolVersion.getProtocols()){var row=new JsonObject();row.addProperty("id",version.getVersion());row.addProperty("name",version.getName());row.addProperty("snapshot",version.isSnapshot());row.addProperty("type",version.getVersionType().name());versions.add(row);}out.add("versions",versions);
  var rows=new JsonArray();try(var jar=new JarFile(args[0])){var entries=jar.entries();while(entries.hasMoreElements()){var entry=entries.nextElement();if(!entry.getName().contains("/mappings-")||!entry.getName().endsWith(".nbt"))continue;byte[] bytes=jar.getInputStream(entry).readAllBytes();var tag=MappingDataLoader.MAPPINGS_READER.read(new java.io.ByteArrayInputStream(bytes));var row=new JsonObject();row.addProperty("file",entry.getName().substring(entry.getName().lastIndexOf('/')+1));row.addProperty("nbt",Base64.getEncoder().encodeToString(bytes));var tables=new JsonObject();for(var pair:tag.entrySet()){if(!(pair.getValue() instanceof CompoundTag table)||table.get("id")==null)continue;try{var mapping=MappingDataLoader.INSTANCE.loadMappings(tag,pair.getKey());if(mapping==null)continue;var values=new JsonArray();for(int i=0;i<mapping.size();i++)values.add(mapping.getNewId(i));var data=new JsonObject();data.addProperty("mapped_size",mapping.mappedSize());data.add("values",values);tables.add(pair.getKey(),data);}catch(RuntimeException failure){var error=new JsonObject();error.addProperty("error",failure.getClass().getSimpleName());tables.add(pair.getKey(),error);System.err.println(entry.getName()+" / "+pair.getKey()+": "+failure);}}row.add("tables",tables);rows.add(row);}}
  out.add("mappings",rows);Files.writeString(Path.of(args[1]),new Gson().toJson(out));System.out.println("Exported "+rows.size()+" ViaVersion mapping files and "+versions.size()+" protocols");
 }
}
