import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.phys.*;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.game.*;
import io.netty.buffer.Unpooled;
public class ExportUse {
 static JsonArray vec(Vec3 v){var a=new JsonArray();a.add(v.x);a.add(v.y);a.add(v.z);return a;}
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var packets=new JsonArray();
  for(var hand:InteractionHand.values())for(var dir:Direction.values())for(boolean inside:new boolean[]{false,true}){
   var pos=new BlockPos(-31,-22,47);var hit=new BlockHitResult(new Vec3(-30.875,-21.25,47.625),dir,pos,inside,true);
   var p=new ServerboundUseItemOnPacket(hand,hit,300);var b=new FriendlyByteBuf(Unpooled.buffer());p.STREAM_CODEC.encode(b,p);
   var bytes=new byte[b.readableBytes()];b.readBytes(bytes);b.release();var r=new JsonObject();r.addProperty("hand",hand.ordinal());r.addProperty("face",dir.get3DDataValue());r.addProperty("inside",inside);r.addProperty("body",Base64.getEncoder().encodeToString(bytes));packets.add(r);
  }
  var rays=new JsonArray();var random=new Random(776);
  for(var block:List.of(Blocks.STONE,Blocks.GLASS,Blocks.OAK_SLAB,Blocks.OAK_STAIRS,Blocks.OAK_FENCE,Blocks.TORCH,Blocks.CHEST,Blocks.LADDER,Blocks.TALL_GRASS)) {
   var state=block.defaultBlockState();var pos=new BlockPos(-31,-22,47);var shape=state.getShape(EmptyBlockGetter.INSTANCE,pos);
   for(int i=0;i<200;i++){
    var center=new Vec3(-30.5,-21.5,47.5);var start=center.add((random.nextDouble()-.5)*4,(random.nextDouble()-.5)*4,(random.nextDouble()-.5)*4);
    var end=center.add((random.nextDouble()-.5)*2,(random.nextDouble()-.5)*2,(random.nextDouble()-.5)*2);
    var hit=shape.clip(start,end,pos);var row=new JsonObject();row.addProperty("state",net.minecraft.world.level.block.Block.getId(state));row.add("start",vec(start));row.add("end",vec(end));
    if(hit!=null){row.add("hit",vec(hit.getLocation()));row.addProperty("face",hit.getDirection().get3DDataValue());row.addProperty("inside",hit.isInside());}
    rays.add(row);
   }
  }
  var root=new JsonObject();root.add("packets",packets);root.add("rays",rays);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
