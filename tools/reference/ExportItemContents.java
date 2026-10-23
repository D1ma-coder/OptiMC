// CPU-only exact-codec fixtures, with nonempty filtered pages and nested templates.
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
import net.minecraft.core.component.*;
import net.minecraft.network.*;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.*;
import net.minecraft.server.network.Filterable;
import io.netty.buffer.Unpooled;
public class ExportItemContents {
 static void fixture(JsonArray rows,String label,ItemStack stack){
  var row=new JsonObject();row.addProperty("label",label);
  row.addProperty("item",BuiltInRegistries.ITEM.getId(stack.getItem()));
  var b=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
  try{ClientboundSetCursorItemPacket.STREAM_CODEC.encode(b,new ClientboundSetCursorItemPacket(stack));var data=new byte[b.readableBytes()];b.readBytes(data);row.addProperty("body",Base64.getEncoder().encodeToString(data));}finally{b.release();}
  rows.add(row);
 }
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var dl=net.minecraft.data.registries.VanillaRegistries.createLookup();var sl=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
  var keys=sl.listRegistryKeys().collect(java.util.stream.Collectors.toSet());var lookup=HolderLookup.Provider.create(java.util.stream.Stream.concat(sl.listRegistries(),dl.listRegistries().filter(r->!keys.contains(r.key()))));
  BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(DataComponentInitializers.PendingComponents::apply);
  var rows=new JsonArray();
  var writable=new ItemStack(Items.WRITABLE_BOOK);writable.set(DataComponents.WRITABLE_BOOK_CONTENT,new WritableBookContent(List.of(Filterable.passThrough("Привет!"),new Filterable<>("raw",Optional.of("filtered")),Filterable.passThrough(""))));fixture(rows,"writable",writable);
  var written=new ItemStack(Items.WRITTEN_BOOK);written.set(DataComponents.WRITTEN_BOOK_CONTENT,new WrittenBookContent(new Filterable<>("Название",Optional.of("Фильтр")),"Автор",2,List.of(Filterable.passThrough(Component.literal("§aСтраница")),new Filterable<>(Component.literal("raw"),Optional.of(Component.literal("filtered").withStyle(net.minecraft.ChatFormatting.GOLD)))),true));fixture(rows,"written",written);
  var renamed=new ItemStack(Items.DIAMOND,23);renamed.set(DataComponents.CUSTOM_NAME,Component.literal("Вложенный алмаз"));
  var shulker=new ItemStack(Items.SHULKER_BOX);shulker.set(DataComponents.CONTAINER,ItemContainerContents.fromItems(List.of(ItemStack.EMPTY,writable,ItemStack.EMPTY,renamed)));fixture(rows,"shulker",shulker);
  var bundle=new ItemStack(Items.BUNDLE);bundle.set(DataComponents.BUNDLE_CONTENTS,new BundleContents(List.of(ItemStackTemplate.fromNonEmptyStack(renamed),ItemStackTemplate.fromNonEmptyStack(written),ItemStackTemplate.fromNonEmptyStack(shulker))));fixture(rows,"bundle",bundle);
  var crossbow=new ItemStack(Items.CROSSBOW);crossbow.set(DataComponents.CHARGED_PROJECTILES,ChargedProjectiles.ofNonEmpty(List.of(new ItemStack(Items.ARROW),new ItemStack(Items.FIREWORK_ROCKET,3))));fixture(rows,"crossbow",crossbow);
  var root=new JsonObject();root.add("stacks",rows);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
