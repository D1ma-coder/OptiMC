// CPU-only visibility oracle; does not initialize Sodium rendering/OpenGL.
import net.caffeinemc.mods.sodium.client.render.chunk.occlusion.DirectionalVisGraph;
import net.minecraft.core.Direction;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
public class ExportVisibility {
 public static void main(String[] args)throws Exception {
  var cases=new JsonArray();
  for(int kind=0;kind<32;kind++){
   var graph=new DirectionalVisGraph();byte[] occupied=new byte[512];int rng=kind+17;
   for(int y=0;y<16;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++){
    rng=rng*1664525+1013904223;
    boolean opaque=switch(kind){case 0->false;case 1->true;case 2->x==8;case 3->y==8;case 4->z==8;case 5->x==0||x==15||y==0||y==15||z==0||z==15;case 6->x==8&&!(y==7&&z==7);case 7->(x+y+z)%2==0;default->Integer.compareUnsigned(rng,kind*100000000)<0;};
    if(opaque){graph.setOpaque(x,y,z);int index=x+z*16+y*256;occupied[index/8]|=(byte)(1<<(index%8));}
   }
   var row=new JsonObject();row.addProperty("opaque",Base64.getEncoder().encodeToString(occupied));var matrices=new JsonArray();for(var visibility:graph.resolve()){long bits=0;for(var a:Direction.values())for(var b:Direction.values())if(visibility.visibilityBetween(a,b))bits|=1L<<(a.get3DDataValue()*6+b.get3DDataValue());matrices.add(bits);}row.add("matrices",matrices);cases.add(row);
  }
  Files.writeString(Path.of(args[0]),new Gson().toJson(cases));System.out.println("Exported 32 directional section visibility cases");
 }
}
