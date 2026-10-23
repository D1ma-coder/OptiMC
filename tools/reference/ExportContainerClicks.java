// Development-only official StreamCodec reference; no game/window/GPU is started.
import com.google.gson.*;import java.nio.file.*;import net.minecraft.SharedConstants;import net.minecraft.server.Bootstrap;import net.minecraft.network.*;import net.minecraft.network.protocol.game.*;import net.minecraft.world.inventory.ContainerInput;import it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap;
public class ExportContainerClicks {
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var packets=new JsonArray();
  for(var kind:ContainerInput.values()) {
   int[] buttons=switch(kind){case SWAP->new int[]{0,1,2,3,4,5,6,7,8,40};case CLONE->new int[]{2};case QUICK_CRAFT->new int[]{0,1,2,4,5,6,8,9,10};default->new int[]{0,1};};
   for(int button:buttons){int slot=kind==ContainerInput.QUICK_CRAFT&&((button&3)!=1)?-999:54;
    var row=new JsonObject();row.addProperty("slot",slot);row.addProperty("button",button);row.addProperty("kind",kind.ordinal());row.addProperty("id",0x12);
    row.addProperty("body",ExportInventory.encode(ServerboundContainerClickPacket.STREAM_CODEC,new ServerboundContainerClickPacket(300,8,(short)slot,(byte)button,kind,new Int2ObjectOpenHashMap<>(),HashedStack.EMPTY)));packets.add(row);
   }
  }
  var root=new JsonObject();root.add("packets",packets);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
