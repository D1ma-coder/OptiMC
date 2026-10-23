// CPU-only vanilla ChestMenu transitions for the visual predictor.
import com.google.gson.*;import java.nio.file.*;import java.util.*;
import net.minecraft.SharedConstants;import net.minecraft.server.Bootstrap;import net.minecraft.core.*;import net.minecraft.core.registries.*;import net.minecraft.core.component.*;import net.minecraft.network.*;import net.minecraft.network.protocol.game.*;import net.minecraft.network.chat.Component;import net.minecraft.world.item.*;import net.minecraft.world.inventory.*;import net.minecraft.world.entity.player.*;
public class ExportContainerPrediction {
 static class DummyLevel extends net.minecraft.client.multiplayer.ClientLevel {
  private DummyLevel(){super(null,null,null,null,0,0,null,false,0L,0);}
  @Override public net.minecraft.world.flag.FeatureFlagSet enabledFeatures(){return net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS;}
 }
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var dl=net.minecraft.data.registries.VanillaRegistries.createLookup();var sl=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);var keys=sl.listRegistryKeys().collect(java.util.stream.Collectors.toSet());var lookup=HolderLookup.Provider.create(java.util.stream.Stream.concat(sl.listRegistries(),dl.listRegistries().filter(r->!keys.contains(r.key()))));BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(DataComponentInitializers.PendingComponents::apply);
  var uf=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");uf.setAccessible(true);var unsafe=(sun.misc.Unsafe)uf.get(null);
  var player=(Player)unsafe.allocateInstance(net.minecraft.client.player.RemotePlayer.class);var eq=new net.minecraft.world.entity.EntityEquipment();var inv=new Inventory(player,eq);var abilities=new Abilities();var level=(DummyLevel)unsafe.allocateInstance(DummyLevel.class);
  for(Class<?> c=player.getClass();c!=null;c=c.getSuperclass())for(var f:c.getDeclaredFields())if(!java.lang.reflect.Modifier.isStatic(f.getModifiers())) {
   if(f.getType()==Inventory.class||f.getType()==Abilities.class||f.getType()==net.minecraft.world.entity.EntityEquipment.class||f.getType()==net.minecraft.world.level.Level.class){f.setAccessible(true);f.set(player,f.getType()==Inventory.class?inv:f.getType()==Abilities.class?abilities:f.getType()==net.minecraft.world.level.Level.class?level:eq);}
  }
  var menu=MenuType.GENERIC_9x3.create(300,inv);menu.getSlot(0).set(new ItemStack(Items.DIAMOND,23));menu.getSlot(3).set(new ItemStack(Items.BREAD,17));menu.getSlot(54).set(new ItemStack(Items.BOOK,9));
  var initial=new JsonArray();ExportInventory.packet(initial,"open",0x3b,ClientboundOpenScreenPacket.STREAM_CODEC,new ClientboundOpenScreenPacket(300,MenuType.GENERIC_9x3,Component.literal("Fixture")));ExportInventory.packet(initial,"content",0x12,ClientboundContainerSetContentPacket.STREAM_CODEC,new ClientboundContainerSetContentPacket(300,8,menu.getItems(),menu.getCarried()));
  var rows=new JsonArray();int[][] actions={{0,0,0},{1,1,0},{2,1,0},{3,0,0},{3,0,1},{0,0,0},{0,0,0},{-999,0,5},{4,1,5},{5,1,5},{6,1,5},{-999,2,5},{0,0,6},{1,0,2},{-999,4,5},{7,5,5},{8,5,5},{-999,6,5}};
  for(int[] a:actions){menu.clicked(a[0],a[1],ContainerInput.values()[a[2]],player);var row=new JsonObject();row.addProperty("slot",a[0]);row.addProperty("button",a[1]);row.addProperty("kind",a[2]);row.addProperty("body",ExportInventory.encode(ClientboundContainerSetContentPacket.STREAM_CODEC,new ClientboundContainerSetContentPacket(300,8,menu.getItems(),menu.getCarried())));rows.add(row);}
  var root=new JsonObject();root.add("initial",initial);root.add("steps",rows);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
